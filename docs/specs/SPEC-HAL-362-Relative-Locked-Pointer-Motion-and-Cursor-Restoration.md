# SPEC-HAL-362: Relative Locked Pointer Motion and Cursor Restoration

**Status:** Adopted / Binding
**Date:** 2026-09-21
**Owner:** Core input / platform boundary
**Runtime scope:** Platform ingress, InputFusion, EventRouter, interaction domains,
and window cursor mediation
**Narrowly extends:** SPEC-HAL-152 Section D with one canonical
relative-pointer event admitted only under the lease defined here
**Depends on:** SPEC-HAL-002, SPEC-HAL-024, SPEC-HAL-033, SPEC-HAL-079,
SPEC-HAL-128, SPEC-HAL-132, SPEC-HAL-139, SPEC-HAL-140, SPEC-HAL-151,
SPEC-HAL-152, SPEC-HAL-356
**Adoption authority:** Delegated architectural acceptance on 2026-09-21,
following the draft and revision reviews.

## Pre-Flight

- Mandatory startup and authority reading completed before adoption:
  `AGENTS.md`, `PRE-FLIGHT.md`, `docs/ENGINEERING.md`, `PROJECT-ENTRY.md`,
  SPEC-HAL-002, SPEC-HAL-002A, DESIGN-INTELLIGENCE, and the governing input,
  focus, OS-ingress, cursor, coordinate, and camera-navigation contracts listed
  above.
- Live baseline inspected at `92e77437` with `winit 0.30.12`.
- Existing hard-capture routing, gesture arbitration, focus teardown,
  InputFusion, cursor hiding/restoration, platform window mediation, and native
  macOS event behavior were inspected before drafting.
- This adoption unit owns only this SPEC. It changes no Rust, application source,
  manifest, changelog, or existing SPEC.
- Governing status is Adopted / Binding. No backend or application may claim
  this capability until its required implementation proof is complete.

## 1. Decision and Scope

Halcyon SHALL provide one optional engine-owned pointer-motion capability named
`RelativeLocked`. It allows a committed, hard-captured pointer gesture to
consume canonical relative logical motion while the host cursor is locked and
hidden. The capability is represented by a short-lived lease and remains
subordinate to the existing gesture, capture, focus, and window-domain owners.

This SPEC governs only:

1. admission and identity of that lease;
2. normalization and routing of relative motion;
3. suppression of duplicate absolute motion during the lease;
4. cursor hiding, optional restoration to the pointer-down anchor, and unlock;
5. typed denial, cancellation, completion, and cleanup behavior; and
6. proof required before a platform backend may claim support.

This SPEC does not create a general pointer framework, global input channel,
application-wide raw-device stream, configurable acceleration engine, cursor
confinement API, cross-window capture, synthetic absolute motion, or semantic
drag policy. It does not define how an application maps a logical delta into a
camera, model, value, or other domain operation.

**INV-362-OPTIONAL:** Applications and widgets that retain the existing
absolute pointer policy observe no new event, state, platform call, latency, or
cursor behavior.

## 2. Relationship to Existing Authority

### 2.1 SPEC-HAL-128 hard capture and gesture arbitration

`RelativeLocked` does not replace pointer capture. A lease may exist only for a
gesture winner that has committed under SPEC-HAL-128 and already owns hard
capture for the same pointer. Candidate registration and threshold arbitration
remain side-effect free. Pointer down alone cannot acquire a lease, lock the
cursor, or hide it.

The existing absolute hard-capture guarantee remains intact: a captured owner
continues to own the corresponding pointer-up even outside window bounds. This
SPEC changes only the motion representation used by an explicitly admitted
gesture between commit and termination.

### 2.2 SPEC-HAL-139 and SPEC-HAL-132 capabilities

`RelativeLocked` is an explicit OS capability within the engine-owned
window/input mediation boundary. Widgets never call `winit`, CoreGraphics, or
another host API. The platform backend reports capability and operation results;
it cannot mutate gesture, capture, focus, or application state directly.

No ambient permission or optimistic support assumption is allowed. Capability
absence and operation denial are ordinary typed outcomes.

### 2.3 SPEC-HAL-140 and SPEC-HAL-151 authority

The lease is explicit, exclusive, and interaction-domain scoped. It belongs to
one focused window and cannot transfer when focus changes. It grants no keyboard
or scroll authority and cannot activate a window. Focus loss cancels the lease
as part of the owning interaction domain's atomic teardown.

### 2.4 SPEC-HAL-152 ingress

Raw host relative motion remains untrusted and platform-specific. It must pass
through receipt, normalization, sanitization, and admission before any core or
consumer observes it. Upon adoption, this SPEC adds exactly one canonical event,
`RelativePointerMotion`, to SPEC-HAL-152 Section D. No raw `winit::DeviceEvent`,
native event, device count, or platform type may cross that boundary.

This is an explicit narrow extension because the current SPEC-HAL-152 canonical
event list has no relative-motion representation. Treating a raw device delta as
an existing logical `PointerMove` would violate SPEC-HAL-152 and is forbidden.

### 2.5 SPEC-HAL-356 absolute baseline

SPEC-HAL-356's routed absolute captured-drag baseline remains unchanged and
portable. Its statement that cursor hiding alone does not lock, warp, recenter,
or synthesize relative motion remains true. This SPEC supplies the separately
accepted platform-ingress vertical anticipated by SPEC-HAL-356 Section 7; it
does not put platform access in a camera controller or make cursor state
semantic authority.

`RelativeLocked` support is additive and capability-gated. A consumer that did
not request it, or a backend that cannot prove it, continues to use the existing
absolute baseline without a false relative-pointer claim.

### 2.6 Coordinate and fusion authority

SPEC-HAL-024 owns physical-to-logical conversion. SPEC-HAL-033 InputFusion owns
canonical input ordering, finite-value admission, motion coalescing, and the
transition from raw ingress to routed events. Consumers receive logical deltas;
they do not receive DPI, device counts, or platform scale factors.

The proposed SPEC-HAL-098 cursor document is adjacent but is not binding
authority for this capability.

## 3. Capability, Lease, and Binding Identity

### 3.1 Motion policy

The existing behavior is named `Absolute`. The new policy is:

```rust
pub enum PointerMotionPolicy {
    Absolute,
    RelativeLocked {
        restore: CursorRestorePolicy,
    },
}

pub enum CursorRestorePolicy {
    None,
    DownAnchor,
}
```

These names specify the contract, not a required source layout. Existing gesture
declarations default to `Absolute`. An additive gesture-policy query or
equivalent static declaration may select `RelativeLocked`; it must not replace
the existing `gesture_surface()` declaration or require changes to consumers
that remain absolute.

`RelativeLocked` always owns a hidden-cursor lease. A visible locked mode is out
of scope. `DownAnchor` is optional; `None` still requires lock, hide, unlock, and
visibility restoration.

### 3.2 Lease identity

Every successful admission mints a nonzero, non-reused
`RelativePointerLeaseId`. The engine stores one immutable binding:

```text
RelativePointerLeaseId
  -> WindowId
  -> InteractionDomainId
  -> PointerId
  -> GestureSessionId and winning GestureCandidateId
  -> TreeWidgetId capture owner
  -> captured PointerButton
  -> CursorRestorePolicy
  -> optional pointer-down Absolute Logical anchor
  -> motion-normalization epoch
```

The normalization epoch records the authoritative scale/conversion state used
to express deltas in logical units. It is engine/platform metadata and is never
semantic application state.

**INV-362-IDENTITY:** A relative event, platform result, release, or cleanup
action is valid only when every binding component and lease generation matches.
Stale results are ignored after idempotent platform cleanup.

**INV-362-EXCLUSIVE:** At most one active lease may exist for a pointer. If the
host exposes only one process-wide pointer lock, at most one lease may exist for
that host seat. A second request is denied; it never steals or transfers the
existing lease.

**INV-362-NO-CROSS-WINDOW:** A lease cannot move to, restore into, or route
motion through another `WindowId` or `InteractionDomainId`.

## 4. Admission

### 4.1 Preconditions

Admission is legal only when all of the following are true:

1. gesture arbitration has committed one winner for the bound pointer;
2. that winner owns SPEC-HAL-128 hard capture for the same pointer and button;
3. the owner is mounted in the bound live interaction domain;
4. the bound window is the focused window for that domain;
5. the candidate declared `RelativeLocked` before commit;
6. the platform reports the required lock, hide, unlock, and normalization
   capabilities, plus cursor positioning when `DownAnchor` is requested; and
7. no conflicting relative-pointer lease exists.

No pre-commit capability probe may lock, hide, move, or otherwise change the
host cursor. A read-only support query is allowed, but it is not admission and
does not reserve future success.

### 4.2 Admission order

The engine performs these steps in order:

1. commit the gesture winner under SPEC-HAL-128;
2. establish hard capture for the winner;
3. validate the complete lease binding and bind the original pointer-down
   anchor already recorded at admitted pointer-down when requested;
4. ask the platform backend for locked relative motion;
5. after lock success, mint and publish the active lease identity internally
   with its relative-ingress gate closed;
6. acquire the owner-bound hidden-cursor lease while that gate remains closed;
   and
7. after both the pointer lock and hidden presentation are established, open
   relative ingress and notify the gesture owner of successful capability
   activation.

The platform lock request therefore occurs only after gesture commitment. No
relative motion may route before hard capture, internal lease publication, and
hidden presentation are all established. Relative backend events arriving after
lock success but before the ingress gate opens are discarded rather than
buffered or routed.

The admission state machine is exactly `Absent -> PlatformLocked ->
ActiveIngressClosed -> ActiveHidden -> Terminating -> Absent`. A failure before
`ActiveHidden` never emits activation. In particular, if hidden-cursor
acquisition fails after internal lease publication, the engine keeps ingress
closed, releases any partial hidden presentation, releases the platform lock,
invalidates the published lease, and cancels through the centralized
SPEC-HAL-128 path with `CursorHideDenied`. Any failure in steps 2-7 releases
partial capture or platform state before returning. The cursor is never hidden
unless lock succeeded.

**INV-362-NO-FALLBACK:** A `RelativeLocked` request cannot silently become an
absolute captured drag, a confined cursor, an unproven recenter loop, or an
unlocked hidden cursor. A declared and proven Section 5.1 locked-center source
is part of the requested relative backend capability. Denial is explicit and
terminal for that gesture session.

## 5. Canonical Relative Motion

### 5.1 Logical delta unit

The only admitted motion value is a finite two-dimensional
`RelativeLogicalDelta` in the bound window's Absolute Logical axis basis:

- positive X follows the existing root-window logical X axis;
- positive Y follows the existing root-window logical Y axis; and
- one unit equals one SPEC-HAL-024 logical unit at the lease's normalization
  epoch.

The delta has no position or origin and grants no hit-test authority. A consumer
may accumulate or semantically map it, but it cannot reinterpret it as a raw
device count or physical pixel.

The canonical ingress event uses the window's logical axis basis. Before widget
delivery, EventRouter presents the vector in the capture owner's Local Logical
basis under SPEC-HAL-024. In the current translation-only pointer route the
numeric vector is unchanged; the distinction prevents absolute-window
coordinates from leaking into widget authority.

Only when a native backend contract identifies its relative source unit as
physical pixels may normalization use the existing SPEC-HAL-024 vector
conversion:

```text
logical_delta = physical_delta / authoritative_scale_factor
```

A backend source already documented in logical units may use a proven identity
conversion. A source expressed in unspecified units, including a generic raw
`winit` delta whose unit is undocumented, MUST NOT be relabelled as logical
pixels. The backend must provide a documented, tested conversion to physical or
logical displacement. If it cannot, `RelativeLocked` is unavailable with
`NormalizationUnavailable`.

When no trustworthy native relative-displacement unit exists, a backend may use
a locked-center absolute-difference source only if the source positions have a
documented physical or logical coordinate unit and the backend proves the
canonical logical displacement from that unit. The differencing, center
maintenance, and filtering of center/warp artifacts remain entirely inside the
platform/InputFusion boundary. That strategy must expose only canonical
relative motion: it cannot expose, synthesize, or route a duplicate semantic
absolute move. It is a declared implementation of `RelativeLocked`, not a
runtime fallback, and it cannot use an empirically tuned coefficient. Without
that proof, the backend reports `NormalizationUnavailable`.

OS acceleration policy may affect the displacement reported by a conforming
backend. This SPEC does not create a second acceleration curve. Given identical
canonical logical events, core routing and consumer-visible results remain
platform independent as required by SPEC-HAL-152.

Any change to authoritative normalization state invalidates the epoch and
cancels the lease before motion under two epochs can mix, including scale,
coordinate basis or transform, source-unit contract, or conversion state.

### 5.2 Canonical event

After normalization and sanitization, InputFusion may produce:

```rust
RelativePointerMotion {
    lease_id: RelativePointerLeaseId,
    window_id: WindowId,
    pointer_id: PointerId,
    time: Instant,
    delta: RelativeLogicalDelta,
}
```

The active lease supplies the window, domain, pointer, and owner association for
raw device motion that lacks a window identifier. Without an active matching
lease, such raw motion is dropped before core routing. An event with a stale
lease, non-finite component, invalid timestamp, wrong pointer, or unavailable
normalization is rejected and cannot mutate state.

### 5.3 Ordering and coalescing

Relative events use the canonical monotonic input order. InputFusion may
coalesce only consecutive motion for the same lease by finite vector addition.
It may not coalesce across pointer-up, focus, cancellation, lifecycle,
scale-factor, window, domain, or lease-generation boundaries.

The final relative delta ordered before a normal pointer-up must be flushed and
routed before that pointer-up. Motion ordered after termination is discarded.

### 5.4 Duplicate absolute suppression

While a lease is active, absolute `CursorMoved` events for the same pointer are
not a second motion source. InputFusion suppresses them before hit testing,
hover, drag update, or consumer routing. They cannot move a virtual cursor,
retarget the gesture, cancel because they cross a window edge, or contribute a
second delta.

Pointer-button release, explicit focus/lifecycle events, and other required
termination signals remain admissible. A compatibility position carried by
pointer-up is termination metadata only. It MUST NOT be differenced against the
last absolute sample, locked center, or `DownAnchor`, and it cannot create,
supplement, or replace a final relative delta. The only final delta is canonical
relative ingress already ordered before pointer-up.

**FORBID-362-DOUBLE-MOTION:** No physical movement may affect the semantic
gesture once through relative input and again through absolute input.

## 6. Routing and Semantic Boundary

EventRouter routes `RelativePointerMotion` only to the lease's matching hard
capture owner. It performs no hit test and does not expose the event to hover,
siblings, unrelated ancestors, another pointer, or another interaction domain.

The relative lease does not change gesture kind, semantic target, binding,
selection, camera, model, or application state. The owning consumer decides how
the canonical logical delta maps through its already committed semantic
association. That mapping must use its governing geometry/view authority and
cannot depend on an empirical pixel coefficient supplied by this capability.

**INV-362-MECHANICAL-ONLY:** Halcyon owns mechanical motion, identity, routing,
cursor mediation, and lifecycle. The consumer owns semantic interpretation and
transactional domain mutation.

## 7. Cursor Anchor and Normal Completion

### 7.1 Down anchor

`DownAnchor` is captured from the original admitted pointer-down, during its
ordinary bounded hit admission and before gesture commitment. Capture is pure
engine data retention: it cannot probe capabilities, lock, hide, warp, or cause
any other platform side effect. After commitment, Section 4.2 binds that exact
recorded value into the lease; the engine cannot resample the cursor at commit
or activation time.

The anchor is stored in the bound window's Absolute Logical content coordinates.
It is not an OS screen-global coordinate and is never valid for another window.

At restoration time the engine verifies that:

- the same window and interaction domain are alive and focused;
- the lease generation still matches;
- the anchor is finite and inside the current client content bounds; and
- the current platform conversion/positioning capability remains valid.

Moving the window does not change the local anchor. A resize that leaves the
anchor outside current client bounds makes restoration unsafe; the engine skips
the warp with a typed result and still unlocks and shows the cursor.

### 7.2 Normal pointer-up order

The compatibility absolute position carried by pointer-up is not an input to
motion calculation. The only final motion is a canonical relative event already
ordered before pointer-up and flushed by step 1.

Normal completion is strictly ordered:

1. flush and route every admitted relative delta ordered before pointer-up;
2. route pointer-up to the SPEC-HAL-128 hard capture owner;
3. let the owner complete its semantic gesture exactly once;
4. while the cursor remains hidden and the lease still owns platform state,
   restore a requested safe `DownAnchor`;
5. ensure the platform pointer lock is released; a backend may combine this
   release with its cursor-positioning operation;
6. release the hidden-cursor lease and restore visible cursor presentation;
7. release the relative-pointer lease and hard capture; and
8. clear the gesture session and all lease bookkeeping.

An anchor-positioning failure does not roll back a semantic operation that
already completed on pointer-up. The engine records a typed restoration failure,
then continues through unlock, visibility restoration, capture release, and
bookkeeping cleanup.

**INV-362-FINAL-DELTA:** Pointer-up cannot overtake or erase the final admitted
relative delta.

**INV-362-NO-STRANDING:** Normal completion cannot leave a logical lock, hidden
cursor owner, relative lease, capture, or gesture session alive after cleanup.

## 8. Cancellation and Failure Semantics

### 8.1 Explicit semantic cancel

Escape or another explicit semantic cancel discards unadmitted pending motion,
notifies the owner once with `ExplicitSemanticCancel`, and rolls back semantic
state under SPEC-HAL-128. If the same window remains live and focused,
`DownAnchor` restoration may run while still hidden and the lease owns platform
state. The engine then ensures unlock, shows, releases capture, and clears the
session.

### 8.2 Forced lifecycle cancel

The following are forced cancellation boundaries:

- window focus loss or domain deactivation;
- capture loss or replacement;
- owner unmount or destruction;
- window destruction;
- application shutdown;
- platform lock loss or security revocation;
- normalization-epoch invalidation; and
- an admitted protocol violation.

Forced cancellation drops pending relative motion, notifies a still-live owner
at most once, clears semantic transient state, unlocks, restores visibility,
releases capture, and invalidates the lease. It does not warp the cursor or
steal focus. Cleanup remains required when the owner callback cannot run.

Window/domain teardown must revoke the lease before the window handle or domain
identity becomes unreachable. Application shutdown must issue the platform
unlock and visibility restoration before final window destruction when the
backend still permits it.

### 8.3 Admission denial

Lock denial, cursor-hide denial, security refusal, missing normalization,
missing requested anchor positioning, stale binding, or unsupported backend
capability produces no surviving active lease and no hidden cursor. Any partial
platform lock is immediately released. The gesture is cancelled with the exact
typed admission reason and cannot retry inside the same session.

### 8.4 Typed outcomes

The implementation must preserve, at minimum, these reason categories:

```rust
pub enum RelativePointerAdmissionError {
    GestureNotCommitted,
    HardCaptureNotOwned,
    OwnerUnavailable,
    WindowNotFocused,
    WindowDomainMismatch,
    ConflictingLease,
    CapabilityUnsupported,
    NormalizationUnavailable,
    RestorationUnsupported,
    PlatformLockDenied,
    CursorHideDenied,
    SecurityDenied,
}

pub enum RelativePointerCancellationReason {
    ExplicitSemanticCancel,
    FocusLost,
    CaptureLost,
    OwnerUnmounted,
    WindowDestroyed,
    ApplicationShutdown,
    PlatformLockLost,
    SecurityRevoked,
    NormalizationChanged,
    InvalidRelativeMotion,
    ProtocolViolation,
}

pub enum CursorRestoreOutcome {
    NotRequested,
    Restored,
    SkippedUnsafeContext,
    FailedPlatformOperation,
}
```

Exact Rust layout may differ, but stringly typed or boolean-only outcomes do not
conform. Backend diagnostics may retain an opaque platform error, but public
engine behavior must branch only on stable Halcyon categories.

`NormalizationChanged` covers every invalidation of the lease's normalization
epoch, including a scale-factor change, coordinate-basis or transform change,
source-unit contract change, or replacement of the authoritative conversion
state. It is not limited to window-size or other window-metrics changes.

The current reasonless `on_gesture_cancel(kind)` callback is insufficient for a
consumer that must distinguish this lifecycle. Conforming implementation must
deliver the typed relative-pointer cancellation reason through a narrow
reason-bearing gesture callback or an owner-only lease lifecycle event before
the owner's private transient state can survive teardown.

## 9. Cleanup and Idempotency

One centralized lease terminator owns restoration, unlock, visibility,
notification, capture release, and record removal. Every normal, denied,
cancelled, unmounted, focus-loss, destruction, and shutdown path uses it.

Cleanup is keyed by `RelativePointerLeaseId` and is idempotent:

- the owner receives at most one completion or cancellation outcome;
- anchor restoration is attempted at most once;
- repeated unlock/show/release requests are harmless;
- a stale platform callback cannot terminate a newer lease; and
- failure of one cleanup action does not skip later actions.

Platform unlock or visibility errors must be diagnosed, but they do not retain
semantic capture or a live relative lease. A backend may issue bounded
best-effort host cleanup during teardown; such cleanup has no input authority
and cannot revive routing.

## 10. Platform Capability Contract

A backend may advertise `RelativeLocked` only when it can prove all operations
required by the requested policy:

| Requirement | `restore: None` | `restore: DownAnchor` |
| --- | --- | --- |
| Acquire exclusive pointer lock | Required | Required |
| Receive continuing relative motion | Required | Required |
| Convert motion to `RelativeLogicalDelta` | Required | Required |
| Hide and show cursor | Required | Required |
| Release lock on every exit | Required | Required |
| Position cursor in the same live window | Not required | Required |

`Confined` does not satisfy `RelativeLocked`. Cursor hiding without lock does not
satisfy it. Absolute outside-window motion does not satisfy it. Support for
`restore: None` does not imply support for `DownAnchor`.

Capability reporting has three distinct states:

1. unsupported or normalization-unproven;
2. supported in principle, with each acquisition still fallible; and
3. an acquisition denied or later revoked by the platform/security context.

No state silently changes the requested motion policy.

### 10.1 Pinned macOS / winit normalization proof

The locked repository dependency is `winit 0.30.12`. Its macOS backend supports
`CursorGrabMode::Locked` through CoreGraphics cursor/mouse disassociation and
supports fallible window-local cursor positioning. It explicitly does not
support `CursorGrabMode::Confined`. macOS drag callbacks continue to emit
absolute `CursorMoved` outside client bounds while a button is held.

For `MouseMoved` and mouse-drag events, the pinned
`src/platform_impl/macos/app.rs::maybe_dispatch_device_event` path reads AppKit
`NSEvent.deltaX` and `NSEvent.deltaY` and forwards both values unchanged as
`winit::event::DeviceEvent::MouseMotion`. Apple defines those properties as
coordinate changes and documents their direct use with point-valued AppKit
window geometry. Apple's high-resolution coordinate contract defines screen
and mouse coordinates in points and reserves backing scale for conversion
between points and backing pixels. The admitted macOS normalization is
therefore the backend-specific identity conversion:

```text
RelativeLogicalDelta.x = NSEvent.deltaX
RelativeLogicalDelta.y = NSEvent.deltaY
```

The result uses the bound window's Halcyon logical-point basis: positive X is
right and positive Y is down. No backing-scale division or empirical
coefficient is permitted. This proof is specific to the audited macOS path in
`winit 0.30.12`; the generic cross-platform `DeviceEvent::MouseMotion` unit
remains unspecified and is not independently admissible.

The source is classified as host-processed and acceleration-unspecified. AppKit
may coalesce the delivered motion, and neither the pinned API contract nor this
SPEC promises accelerated or unaccelerated samples. OS acceleration may
therefore affect the canonical displacement under Section 5.1, but Halcyon adds
no second acceleration curve and exposes no raw, native, or user-visible
sensitivity coefficient.

The normalization provenance is the tuple of macOS, `NSEvent.deltaX/Y`, the
audited `winit 0.30.12` adapter path, the AppKit logical-point coordinate basis,
and identity conversion. A change to any member invalidates the normalization
epoch and cancels the lease before motion from two epochs can mix.

The pinned macOS cursor-positioning implementation re-associates mouse and
cursor position as part of the warp. It can therefore satisfy the ordering only
as a backend operation that restores while hidden and also releases the lock;
the subsequent unlock step must be idempotent.

macOS support remains unclaimed until the pinned source audit, the production
relative ingress path with no scale arithmetic, and real-backend capture at one
backing scale prove this identity normalization in both unlocked and
`CursorGrabMode::Locked` operation. The capture must verify axis signs,
continuing deltas while locked, and constant absolute location under
cursor/mouse disassociation. A second stable backing-scale capture does not add
conversion proof to this point-unit identity path. Any mismatch observed at any
scale fails conformance. Scale-change cleanup and multi-display no-stranding
remain separate platform obligations under Section 12.2. Failure of any check
makes the capability unavailable with `NormalizationUnavailable`; it does not
authorize a tuned coefficient or a raw/native-delta escape hatch.

## 11. Forbidden Behaviors

**FORBID-362-PRECOMMIT:** No lock, hide, warp, or relative-motion admission from
pointer down, hover, focus, candidate registration, or threshold detection.

**FORBID-362-RAW-LEAK:** No raw `winit`, native event, device count, scale factor,
or platform enum outside the platform/InputFusion boundary.

**FORBID-362-UNIT-RELABELLING:** No treating an unspecified raw delta as a
logical pixel, physical pixel, or normalized logical delta.

**FORBID-362-SILENT-FALLBACK:** No fallback from `RelativeLocked` to an ordinary
absolute drag, confinement, an unproven recenter loop, or hidden-only behavior.
A declared locked-center acquisition implementation that conforms to Sections
5.1 and 10 is backend relative-motion machinery; it does not permit an absolute
move to escape the platform/InputFusion boundary.

**FORBID-362-CROSS-DOMAIN:** No global capture, cross-window lease, background
window acquisition, or routing based on whichever window is currently visible.

**FORBID-362-DIRECT-OS:** No widget, controller, application consumer, camera, model,
or renderer call to `winit` or a host pointer API.

**FORBID-362-SEMANTIC-ENGINE:** No engine-owned sensitivity constant, camera
operation, model operation, target selection, or domain mutation derived from
relative motion.

**FORBID-362-DUPLICATE:** No absolute move routed as gesture motion while the
same pointer has an active relative lease.

**FORBID-362-CURSOR-LEAK:** No denial, error, cancel, focus loss, unmount,
destruction, or shutdown path may intentionally retain a hidden cursor or lock.

**FORBID-362-WARP-ON-FOCUS-LOSS:** No anchor warp after focus loss, window
destruction, security revocation, or another unsafe context.

## 12. Required Proof

### 12.1 Engine and fake-backend proof

1. Existing absolute gesture declarations perform no relative capability query
   or platform operation.
2. A relative request before gesture commit is rejected without lock or hide.
3. A committed winner with matching hard capture acquires exactly one bound
   lease; every binding mismatch is denied.
4. Successful ordering is lock success, internal active-lease publication with
   ingress closed, hidden-cursor acquisition, ingress opening, then activation
   notification; normal release ordering matches Section 7.2 exactly.
5. Lock/security denial never hides. Hidden-cursor acquisition failure after
   lock keeps ingress closed, unlocks, invalidates the lease, and cancels the
   gesture once with the exact typed reason.
6. Duplicate absolute motion is suppressed while matching relative motion is
   delivered owner-only.
7. Relative input with no lease, stale lease, wrong pointer/window/domain,
   non-finite delta, or invalid timestamp is rejected without mutation.
8. Coalescing preserves finite vector sum and never crosses an ordering barrier;
   the final delta is delivered exactly once before pointer-up, and pointer-up's
   compatibility absolute position creates no motion.
9. Focus loss drops pending motion, performs no warp, unlocks/shows, cancels the
   owner once, and leaves no capture, session, cursor owner, or lease.
10. Explicit semantic cancel uses safe requested restoration; owner unmount,
    window destruction, and shutdown remain callback-independent and idempotent.
11. Restore failure records `FailedPlatformOperation` while unlock, show,
    capture release, and successful semantic completion remain intact.
12. A stale completion or cleanup callback cannot affect a newer lease.
13. Any `NormalizationChanged` boundary cancels before deltas from two
    normalization epochs can mix.
14. Existing SPEC-HAL-128 absolute outside-window move/up routing and
    SPEC-HAL-356 cursor behavior remain unchanged.

### 12.2 Platform proof

Each supported backend must prove the following on the real backend, except for
the narrowly defined unforceable native-denial stimulus in item 1:

1. lock acquisition and release, including denial;
2. continuing relative motion at every window edge;
3. the named native motion source and documented source unit, or the exact
   locked-center absolute-difference source and unit, plus conversion to logical
   deltas without empirical coefficients. A source in physical pixels or a
   scale-dependent conversion requires real capture at two scale factors. A
   documented logical-point identity source requires a source audit, proof that
   the production relative path applies no scale arithmetic, and real locked
   and unlocked capture at one backing scale;
4. no duplicate semantic motion from simultaneous absolute events;
5. safe same-window `DownAnchor` restoration when claimed;
6. focus loss, application switching, unmount, window destruction, and shutdown
   cleanup; and
7. multi-display behavior showing that the hidden locked pointer cannot be
   stranded on another display and that unsafe restoration does not warp.

For item 1, real lock acquisition and release remain mandatory. A genuine
native lock refusal must be recorded and correctly handled when it occurs or a
verified safe trigger can produce it. If the pinned host API exposes no safe,
deterministic public trigger and no refusal occurs naturally, the backend must
document that limitation from the audited native call and prove the same
adapter-to-engine denial path deterministically under Section 12.1: typed
`PlatformLockDenied`, no cursor hide or surviving lease, and complete capture
and session cleanup. This substitutes only the unavailable host-denial
stimulus; it does not waive real acquisition or release, cleanup, or any other
item in this section. Duplicate-lock, wrong-window, unsupported-mode, and
injected-error results are not genuine native refusals. Any genuine refusal
observed during conformance must be evaluated and cannot be dismissed under
this provision.

### 12.3 Consumer proof

Cancelling an already-admitted drag because an absolute position leaves
`visible_rect` is a consumer defect and MUST be removed during integration.
This correction does not widen initial hit admission: the original pointer-down
must still pass the ordinary bounded hit test in item 1. After relative
activation, `visible_rect` and suppressed absolute motion are not cancellation
authority.

A consumer test must show that:

1. semantic admission still begins from an ordinary bounded pointer-down hit;
2. relative movement continues after the physical pointer would have crossed
   viewport and window bounds;
3. canonical logical deltas are mapped through the consumer's committed
   geometry/view association without a platform or pixel coefficient;
4. target/association invalidation cancels atomically;
5. the final delta is applied once before completion; and
6. every typed cancellation clears the consumer's private transient state.

## 13. Adoption and Compatibility Boundary

This SPEC is adopted and binding. Adoption authorizes the narrow capability and
the one canonical event defined here; it does not by itself prove any backend
supported or authorize an application to bypass the required implementation
evidence.

Before implementation acceptance:

- SPEC-HAL-152's canonical event model must acknowledge this narrow extension;
- capability results and cancellation reasons must be typed;
- at least one real backend must satisfy Section 12.2; and
- the aggregate and focused verification results, including inherited failures,
  must be recorded against the implementation revision.

The existing absolute pointer path remains the portable required baseline.
Relative locked motion is an optional additional capability with explicit
admission and no silent downgrade.
