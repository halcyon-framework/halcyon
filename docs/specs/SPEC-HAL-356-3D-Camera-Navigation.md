# SPEC-HAL-356: 3D Camera Navigation

## Pre-Flight

- SPEC: SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-021, SPEC-HAL-033, SPEC-HAL-036, SPEC-HAL-038, SPEC-HAL-042, SPEC-HAL-140, SPEC-HAL-142, SPEC-HAL-151, SPEC-HAL-152, SPEC-HAL-328, SPEC-HAL-343, SPEC-HAL-354, and SPEC-HAL-355 read before writing this task: YES
- Policy: Interactive — optional application/widget camera policy over an ordinary retained `Viewport3D`
- Interaction added: YES — explicit pointer gestures and keyboard-capture-gated camera navigation are authorized by this SPEC
- Forbidden behaviors checked: FORBID-356-RENDERER-CONTROLLER, FORBID-356-MODEL-MUTATION, FORBID-356-SECOND-CLOCK, FORBID-356-REPEAT-MOTION, FORBID-356-IMPLICIT-CAPTURE, FORBID-356-NAVIGATION-LEAKAGE, FORBID-356-FLY-POINTER-MONOPOLY, FORBID-356-CURSOR-LEAK, FORBID-356-PLATFORM-SHORTCUT, FORBID-356-FALSE-RELATIVE-POINTER, FORBID-356-UNNORMALIZED-WHEEL, FORBID-356-UNBOUNDED-MOTION, FORBID-356-2D-CONTAMINATION, FORBID-356-TRANSFORM-INFERENCE
- Existing infrastructure: immutable `ViewDescriptor3`; checked `RigidPose3`, `Projection3`, and world-space math; mechanical `Viewport3D::set_request`; retained widget event/update lifecycle; EventRouter focus and independent pointer/keyboard capture; owner-addressable `EventContext` cursor requests, sticky `CursorIcon::Hidden` application, and platform cursor visibility; `KeyboardAuthorityMode`; canonical key/pointer events; InputFusion line/pixel normalization into additive logical-pixel scroll deltas; `EngineTicker` frame `dt`; ordinary invalidation; Timeline interpolation; Showcase 3D orbit demo
- Reinvention risk: Adds one optional pure camera state/controller policy and reuses the existing router, capture domains, widget update phase, frame clock, 3D math, immutable view request, and Showcase. It does not add a renderer camera, second input system, second scheduler, 3D scene graph, platform event path, or generic 2D/3D spatial framework.

**Status:** Adopted / Binding
**Date:** 2026-08-24
**Layer:** Public 3D presentation state / optional widget interaction policy
**Owner:** 3D Camera Navigation Architecture
**Depends on:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-021, SPEC-HAL-033, SPEC-HAL-036, SPEC-HAL-038, SPEC-HAL-042, SPEC-HAL-140, SPEC-HAL-142, SPEC-HAL-151, SPEC-HAL-152, SPEC-HAL-328, SPEC-HAL-343, SPEC-HAL-354, SPEC-HAL-355
**Extends without amending:** SPEC-HAL-355 durable camera/controller non-goal by supplying the separately authorized application/widget policy
**Binding adjacency:** SPEC-HAL-357 adds local-domain/content composition while preserving this SPEC's camera-only operation and target boundary
**Implementation status:** Implemented: checked controller wheel-transition boundary, Showcase-owned smooth wheel zoom, the scoped Showcase geometric-anchor prototype, the explicit Showcase named-figure local-Z transform consumer, and bounded Fly velocity response

This document is adopted and binding. It authorizes first-class reusable 3D
camera navigation without transferring camera, input, timing, or semantic
authority into `Viewport3D` or the renderer.

---

## 1. Purpose and Authority Decision

Halcyon has a first-class immutable 3D presentation request, but the rendering
substrate deliberately does not own a durable camera or controller. Applications
need a reusable policy for orbit, true camera pan, perspective optical zoom,
orthographic span zoom, and optional game-style fly motion.

The authority decision is:

```text
canonical routed input + explicit interaction-domain capture
    -> optional CameraNavigationController3 owned by a retained widget/application
    -> camera-only CameraState3 mutation during event/update authority
    -> immutable ViewDescriptor3 with a new view revision
    -> existing Viewport3D request/publication path
```

`CameraNavigationController3` is a presentation controller. It may mutate only
its own checked camera state and transient navigation state. It never mutates
`ModelState`, `RenderSnapshot`, geometry, model placements, the WidgetTree, or
renderer-owned resources.

Camera navigation is classified by its checked operation and target, never by a
visual result. `OrbitRotate`, `FreeLook`, `CameraPan`, Fly translation,
Perspective optical FOV zoom, and orthographic span zoom all target
`CameraState3`. A physical camera dolly would likewise target camera state, but
is not admitted by this baseline. None of these operations rotates, translates,
or scales a model placement. A visible background, a skybox-like plane, or an
apparently stationary figure is not evidence of an operation or target.

Transform interaction is a distinct application policy: `TransformRotate`,
`TransformTranslate`, and `TransformScale` require an explicit
application-resolved transform target and publish a replacement complete
`ModelState`. This SPEC owns neither that target source nor its bindings. It
does not define a `rotate_scene` operation: a consumer may choose a whole-model
root, a named figure, or one placement as its explicit target only under its own
policy.

Binding SPEC-HAL-357 establishes explicit local-domain/content IDs, similarity
embeddings, and committed mechanical association. Those values create no camera
operation, target, pivot, binding, or navigation authority. A domain embedding
or content-placement change is still a model/composition operation:
`CameraNavigationController3` remains unaware of domain identity and navigation
remains view-only.


Fly navigation is concurrent with ordinary scene interaction. Keyboard capture
authorizes held Fly motion independently of the short-lived pointer capture used
for a camera gesture. The default Fly pointer policy claims secondary-button
drag for look and auxiliary-button drag for pan; it does not claim primary-button
input. Primary down, drag, up, and click therefore remain available to the
consumer's scene hit-test, selection, and manipulation policy while Fly remains
active, including while movement keys are held where EventRouter routing permits.

---

## 2. Ownership Matrix

| Concern | Sole owner | Camera navigation role |
|---|---|---|
| Camera pose, projection, orbit anchor, mode | Retained application/widget controller state | Mutates checked camera state and emits an immutable view |
| Camera key, pointer, scroll, sensitivity, and bounds policy | `CameraNavigationPolicy3` supplied by the application/widget | Maps each admitted binding to one named camera operation targeting `CameraState3` |
| Keyboard focus and capture | Existing EventRouter interaction domain | Controller consumes routed keys only while authority mode is `Capture` |
| Pointer capture | Existing EventRouter interaction domain | Widget requests/releases capture only around the camera gesture selected by policy |
| Cursor visibility during a captured drag | Active retained gesture owner, mechanically enforced by EventRouter and platform cursor application | Camera consumer owns only its active camera gesture; a scene/object layer owns only its own active drag |
| Ordinary scene pointer interaction | Retained scene/application consumer | Receives every pointer button the active camera policy does not claim; default Fly leaves primary input unclaimed |
| Window-focus teardown | Existing interaction-domain lifecycle | Controller clears transient input when capture/focus is released |
| Canonical key/pointer normalization | Existing platform ingress and input fusion | Controller consumes canonical events only |
| Held-motion time and cadence | Existing frame loop / `EngineTicker` / widget `update(dt)` | Integrates movement from supplied `dt`; may request continued frames |
| Optional interpolation/easing | Existing Timeline owned by the caller | May animate explicit camera transitions; never drives held input |
| Immutable render view | `ViewDescriptor3` | Controller constructs the next checked descriptor |
| 3D rendering and publication | `Viewport3D` / `Context3D` / custom-render lifecycle | Mechanical consumer only; owns no navigation state or policy |
| Model/world transforms | Application model owner | Never changed by camera navigation |
| Transform target identity, group membership, selection, and transform bindings | Application consumer | Resolves an explicit target and publishes a replacement complete `ModelState`; never inferred from rendering |

No owner in this table may be duplicated for convenience.

---

## 3. Coordinate and Camera-State Contract

### 3.1 Coordinate Convention

Navigation uses the existing right-handed 3D convention:

- camera-local right is `+X`;
- camera-local up is `+Y`;
- camera-local forward is `-Z`;
- yaw rotates around world `+Y`;
- pitch rotates around the yawed camera-local `+X` axis;
- roll is not introduced by the baseline controller.

Yaw and pitch are finite radians. Yaw is normalized to a bounded canonical
interval. Pitch is clamped to the policy's finite open-pole range so the camera
never crosses or reaches a singular vertical orientation.

### 3.2 Canonical State

`CameraState3` is checked pure presentation data containing:

- camera world position;
- finite yaw and bounded pitch;
- a world-space orbit target;
- a finite positive orbit distance;
- the checked `Projection3`.

The state always yields one `RigidPose3` and one `ViewDescriptor3`. The pose is
canonical presentation truth. Orbit target and distance are controller metadata,
not model geometry and not renderer state.

In Orbit mode, position is derived from target, forward direction, and distance.
In Fly mode, movement translates both position and the retained orbit target;
look changes re-anchor the target along the new forward ray at the retained orbit
distance. This keeps the metadata coherent and makes mode switching
pose-preserving.

All state constructors and operations are transactional: invalid or
non-representable input returns an error without partially mutating state.

### 3.3 Projection Ownership

Projection is part of camera presentation state. Switching between checked
perspective and orthographic projections changes only the emitted view.

- Perspective wheel input performs optical zoom by changing the checked vertical
  field of view. Centre-only zoom and the existing checked anchored target
  operation remain bounded by §§9.3–9.3A.
- Orthographic wheel input changes the checked vertical span and does not fake
  zoom by moving a camera whose projection scale is distance-independent.

Projection near/far semantics remain exactly those of SPEC-HAL-355.

---

## 4. Modes and Pose-Preserving Switching

The baseline controller exposes exactly two policies:

```text
CameraNavigationMode3::Orbit
CameraNavigationMode3::Fly
```

Orbit supports rotate-about-target, true camera pan, optical/span zoom, and reset.
Fly supports yaw/pitch look, forward/back, strafe left/right, true camera pan,
optical/span zoom, and reset. It has no world-vertical held-key translation.

Fly mode is not a mutually exclusive viewport interaction mode. Held Fly keys,
camera-look pointer capture, and an ordinary scene pointer gesture are separate
interaction concerns. The camera controller may integrate held motion while no
camera pointer gesture exists, and switching Fly on never claims primary pointer
input by itself.

Mode switching:

1. preserves current camera pose and projection;
2. clears every held key and active pointer gesture before the new policy is
   active;
3. keeps keyboard focus/capture ownership with the EventRouter rather than
   silently entering or releasing capture;
4. re-anchors Orbit metadata along the current forward ray when required; and
5. emits no model or geometry change.

Orbit to Fly and Fly to Orbit therefore have no visual jump before a subsequent
explicit navigation input.

---

## 5. Explicit Policy and Default Bindings

Navigation behavior is supplied through a checked
`CameraNavigationPolicy3`. `Viewport3D` and the renderer contain no key names,
mouse gestures, speeds, or mode logic.

The default policy is:

| Input | Orbit binding | Fly binding | Named operation | Target source / policy |
|---|---|---|---|---|
| Primary drag | admitted | unclaimed by camera navigation | `CameraPointerOperation3::OrbitRotate` | `CameraState3` through `CameraNavigationPolicy3` |
| Secondary drag | unclaimed by camera navigation | admitted when Space is not held | `CameraPointerOperation3::FreeLook` | `CameraState3` through `CameraNavigationPolicy3` |
| Space + secondary drag | unclaimed by camera navigation | admitted | `CameraPointerOperation3::OrbitRotate` | current explicit `CameraState3::orbit_target` and retained orbit distance through `CameraNavigationPolicy3` |
| Auxiliary drag | admitted | admitted | `CameraPointerOperation3::CameraPan` | `CameraState3` through `CameraNavigationPolicy3` |
| Wheel | admitted | admitted | `CameraWheelOperation3::PerspectiveOpticalFovZoom` or `::OrthographicSpanZoom`, selected only by checked projection | `CameraState3` through `CameraNavigationPolicy3` |
| `W` / `S` | unbound | admitted | `FlyMotionOperation3::Forward` / `::Backward` | `CameraState3` through `CameraNavigationPolicy3` |
| `A` / `D` | unbound | admitted | `FlyMotionOperation3::StrafeLeft` / `::StrafeRight` | `CameraState3` through `CameraNavigationPolicy3` |
| Space | unbound | admitted only as a held pointer modifier | `FlyMotionOperation3::OrbitModifier` | resolves the Fly secondary-drag operation only; it is not camera translation |
| Shift | unbound | admitted | `FlyMotionOperation3::SpeedModifier` | `CameraState3` through `CameraNavigationPolicy3` |
| Escape | EventRouter capture-leave lifecycle | EventRouter capture-leave lifecycle | `CameraNavigationController3::cancel_input` | Controller transient-input state under EventRouter lifecycle; never a camera transform target |

The pointer buttons and Fly keys are configuration, not renderer or Showcase
constants. Within each mode, the rotate/look and pan buttons must be distinct.
The default Orbit and Fly look buttons may differ without changing camera state
or requiring the user to disable Fly before interacting with scene objects. In
default Fly, Space is a captured modifier only: it never contributes a held
translation axis. Its state at secondary down resolves that drag to `OrbitRotate`
or `FreeLook`; the accepted operation remains fixed for that pointer-capture
lifetime, even if Space is released before pointer up.


Every camera binding resolves to exactly one named operation and the camera
target above, or is unclaimed by camera navigation. A physical `CameraDolly`
means translation of the camera position along its forward axis relative to a
camera-owned target. It is distinct from `CameraPan`, Fly translation, optical
FOV zoom, orthographic span zoom, and every model transform, but is not bound or
implemented by this baseline. A consumer must not substitute it because a scene
looks like a skybox or because a visual result resembles magnification.

The current canonical
`KeyCode::Character` ingress is logical-layout-first. This SPEC does not claim
physical-key-layout portability. A future physical-key binding surface must be
added at canonical input ingress and then consumed through the same policy; it
must not be read directly from a platform event by this controller.

Policy parameters are finite and checked, including look sensitivity, pan scale,
Fly target speed, speed multiplier, Fly acceleration/deceleration response rates
in `s^-1`, pitch limit, zoom response per canonical logical scroll pixel,
relative minimum perspective `tan(vertical_fov / 2)` and orthographic minimum/
maximum span factors, maximum pointer delta, maximum logical-pixel scroll delta
per fused event, and maximum admitted frame `dt`.

The Perspective minimum-tangent factor is relative to the checked initial/default
Perspective projection admitted by the controller. That projection's tangent is
the exact finite maximum/wide endpoint; Perspective wheel input may return to it
but never widen past it. Orthographic minimum/maximum span factors remain relative
to its admitted reference span. The controller derives and exposes finite absolute
bounds from these references, keeping policy scale-independent without admitting
an over-wide Perspective endpoint.

---

## 6. Focus, Capture, and Routing Lifecycle

### 6.1 Entry

The controller never enters focus or capture itself. A consuming widget uses the
existing `EventContext` API.

On a pointer down mapped to a camera gesture by the active checked policy, an
interactive consumer:

1. requests focus;
2. requests keyboard capture;
3. requests pointer capture for that gesture's button; and
4. begins the matching controller gesture only after the routed target event.

For the default Fly secondary-button look, the consumer also requests
`CursorIcon::Hidden` in the same routed entry transaction. EventRouter may admit
that hidden request only after the same widget owns the pointer capture for the
active gesture. A hidden request does not grant capture, and a denied capture
must not produce even a transient hidden cursor.

Scroll over the viewport may adjust the camera without stealing keyboard focus
or capture. In default Fly, primary pointer input is not a camera gesture and the
consumer must return it to the ordinary scene interaction path. Secondary drag
is camera look only for that gesture's pointer-capture lifetime. Its hidden
cursor has exactly the same lifetime. Fly selection, hover, focus, keyboard
capture, held motion, secondary down without admitted pointer capture, and
primary input are not cursor-hide authority.

Where an explicit consumer entry request uses the existing deferred focus/capture
route, scheduling is not acknowledgement: the mounted viewport's actual
`on_capture_enter` callback establishes entry completion. Any pending entry
record belongs to that request alone. Escape, blur and ordinary capture leave
must not recreate it or silently recapture keyboard authority. No raw key path,
screen polling or direct interaction-domain mutation is admitted. This contract
does not select an application control, initial mode or retry implementation.

### 6.2 Keyboard Authority

Focused-only Navigation mode never authorizes camera keys. The controller must
return unhandled and make no state change unless the supplied
`KeyboardAuthorityMode` is `Capture`.

While captured, bound keys are consumed by the controller. Unbound keys and all
text-input events remain unhandled. No global shortcut, screen-level polling, or
raw platform event may bypass this rule.

### 6.3 Exit and Cancellation

Capture/focus lifecycle remains governed by SPEC-HAL-151 and SPEC-HAL-328:

- Escape unconditionally releases keyboard capture while preserving focus;
- focus movement, window focus loss, widget destruction, and unmount release
  capture according to existing interaction-domain rules;
- keyboard, pointer, and scroll capture remain distinct domains.

On keyboard-capture leave, focus loss, or unmount, the consumer must clear the
controller's complete transient input state and release any active pointer
capture. A later frame cannot continue motion from a stale key or gesture.

A mode or projection control change during a camera or independently owned
scene/object drag must use the routed focus/capture boundary above, not merely
clear controller or consumer update state. That boundary releases pointer
capture, restores any owned hidden cursor, clears transient gesture/held state,
and prevents the next routed move from continuing the old operation under the
new mode. Distinct cursor and keyboard owners remain unchanged.

Pointer up ends the active gesture and releases pointer capture. It does not
clear held Fly keys or implicitly release keyboard capture; the user may continue
keyboard navigation and ordinary primary-button scene interaction until Escape
or an existing focus/capture transition.

Cursor visibility restoration is mandatory and idempotent on every path that
ends or prevents the hidden camera gesture:

1. matching secondary-button up or normal drag completion;
2. controller rejection, pointer-processing failure, or explicit gesture cancel;
3. pointer-capture denial, release, replacement, router cancellation, invalid
   target, or cancelled drag completion;
4. keyboard-capture leave, including Escape and Tab transitions;
5. focus transfer, widget blur, or window focus loss;
6. widget unmount or destruction; and
7. application shutdown of the owning window.

The active gesture owner requests visible cursor restoration on callback-bearing
exits. EventRouter must mechanically restore visibility when capture or owner
teardown bypasses such a callback. Restoration releases only the hidden-cursor
lease; ordinary cursor routing then chooses the visible icon. No stale hidden
state may survive after the owning capture is gone.

### 6.4 Accessibility and Text Input

Keyboard capture is explicit and escapable. Text editors, IME composition,
screen-level navigation and unrelated focused widgets retain their existing
authority because camera bindings are inactive outside the viewport's Capture
mode. Application controls and their presentation are consumer policy.

This SPEC does not claim a new accessibility semantic role or platform relative
pointer capability. Those require their owning authorities.

---

## 7. Pointer Gestures and Platform Capability

The portable baseline uses Halcyon's existing routed pointer events and pointer
capture. Pointer motion is computed from successive finite widget-local absolute
positions while a gesture is active.

The checked pointer binding policy resolves `(mode, button)` to at most one
camera gesture. Default resolution is:

```text
Orbit + Primary               -> OrbitRotate
Orbit + Auxiliary             -> CameraPan
Fly   + Secondary             -> FreeLook
Fly   + Space + Secondary     -> OrbitRotate
Fly   + Auxiliary             -> CameraPan
Fly   + Primary               -> none (ordinary scene path)
```

Only a resolved camera gesture may request camera-owned pointer capture. Fly
activation, hover, focus, held keys, and ordinary primary input request none.

Current native ingress exposes no raw relative mouse motion and no cursor
confinement/pointer lock. It does expose canonical widget cursor requests and
platform cursor visibility. Therefore:

- Fly look is pointer-captured drag, not unbounded mouse-look;
- the pure controller does not access cursor or platform state;
- the camera consumer hides the cursor only for its admitted default Fly
  secondary-button look capture and restores it under §6.3;
- cursor hiding does not warp, recenter, confine, lock, or synthesize relative
  motion;
- reaching a platform/window edge may end available absolute motion; and
- no support claim may describe the baseline as relative-pointer or FPS mouse
  lock.

In default Fly, primary input remains outside camera navigation. If a retained
scene/object layer begins its own primary-button drag, that layer may use the same
capture-owner-bound hide/restore lifecycle for only that active drag. The camera
consumer must neither request cursor hiding for that primary input nor retain or
restore the scene layer's cursor lease. If no scene/object drag becomes active,
primary down, move, up, and click produce no hidden cursor from this vertical.


A later relative-pointer vertical belongs in platform ingress, canonical input
normalization, interaction-domain capture semantics, and accessibility review.
Once normalized, it may feed the same checked look operation. It must not create
a controller-private platform path or make OS cursor-grab state semantic
authority.

Pointer deltas are finite and bounded by policy before rotation or translation.
Non-finite input is rejected without mutation.

---

## 8. Held-Key Motion and Timing

Held-key motion is controller-owned target velocity plus engine-frame integration,
never key-repeat integration or a second scheduler.

1. A non-repeat captured `KeyDown` changes one held-key bit.
2. A captured `KeyUp` clears that bit.
3. Repeated key events may be consumed but never add distance, velocity, or
   another state transition.
4. The controller normalizes combined held movement axes so diagonal input does
   not exceed configured scalar target speed. The default target speed is
   `3.0` world units per second (calibrated from the prior `2.5`, a 20% increase);
   the checked `4.0` Shift multiplier therefore targets `12.0` units per second.
5. The controller retains only its current Fly velocity. While a movement axis is
   held, velocity approaches that target with the finite checked acceleration
   response rate; with no movement axis held, it approaches zero with the finite
   checked deceleration response rate. `OrbitModifier` contributes no target
   velocity.
6. The widget requests a frame on every held-state transition, and
   `Widget::needs_update` remains true while either a movement target or retained
   nonzero velocity remains.
7. Each admitted `Widget::update(dt)` calls the controller once with the
   engine-supplied `dt`. For target velocity `v_t`, current velocity `v_0`,
   selected response rate `r`, and admitted interval `d`, it uses the exact
   exponential-response step:

   ```text
   v_1 = v_t + (v_0 - v_t) * exp(-r * d)
   delta_position = v_t * d + (v_0 - v_t) * (1 - exp(-r * d)) / r
   ```

   It deterministically snaps only a residual at or below `0.001` world units
   per second to zero so a released key stops requesting frames. The default response rates are
   `30 s^-1` acceleration (95% target response in 100 ms) and `40 s^-1`
   deceleration (95% zero response in 75 ms).
8. Reset, a mode transition, keyboard-capture leave, focus loss, unmount, and
   explicit cancellation clear held state and retained velocity before any later
   update may translate the camera.

The controller has no `Instant`, wall clock, frame counter, Timeline, worker,
timer, or private scheduler. Non-finite or negative `dt` is rejected. Admitted
`dt` is capped by the checked policy bound to prevent a stall from producing an
unbounded teleport. For equal input-transition schedule and elapsed time within
that admitted bound, the exact step produces cadence-independent displacement
within the declared floating-point tolerance.

Timeline remains available to a caller for explicit camera transitions, but it
does not own held motion and does not become a second camera authority.

---

## 9. Camera Operations

All baseline operations are composable checked mutations of `CameraState3`:

### 9.1 Orbit Rotate / Free Look

Finite pointer delta updates yaw and pitch under policy sensitivity and bounds.
`CameraPointerOperation3::OrbitRotate` recomputes position around its unchanged
explicit `CameraState3::orbit_target` at its retained distance. In default Fly,
only Space held at secondary down resolves this tether operation; it neither
selects a scene object nor reads selector, wheel-anchor, ray, geometry, or
rendered output. `CameraPointerOperation3::FreeLook` keeps position fixed and
re-anchors the retained target on the new forward ray.

### 9.2 Camera Pan

`CameraPointerOperation3::CameraPan` translates both camera position and orbit
target along camera right/up axes. It never applies an inverse transform to the
scene and never changes model placements. Pan scale is finite, bounded, and
proportional to retained orbit distance so interaction remains usable across
scale.

### 9.3 Perspective Optical FOV Zoom

Physical wheel movement reaches this controller only as an already normalized
canonical logical-pixel scroll delta. For perspective projection, the controller
maps that delta to an optical zoom by multiplying the positive half-angle tangent:

```text
t = tan(vertical_fov_radians / 2)
t' = clamp(t * exp(-delta_y * exponent), minimum_tangent, default_tangent)
vertical_fov_radians' = 2 * atan(t')
```

`minimum_tangent` is the finite positive lower bound derived from the admitted
initial/default Perspective projection. Its `default_tangent` is the exact upper/
wide endpoint. Positive scroll narrows FOV toward the minimum; negative scroll may
only restore it toward `default_tangent` and clamps there, even after arbitrary
outward input. The controller reconstructs the checked perspective descriptor with
the original near/far planes. In both navigation modes, centre-only wheel FOV zoom
leaves camera position, orientation, yaw, pitch, orbit target, and orbit distance
unchanged. It is an optical projection change, not a camera dolly, FOV-angle
multiplier, target translation, or model/world mutation.

### 9.3A Checked Anchored Target Boundary

The existing anchored target operation accepts one explicit world point. Its
checked numeric compensation does not provide an anchor source, generic picking
or evidence that a geometric point is rendered-visible.

An existing consumer that derives its anchor from the current presentation must
retain these currentness gates before the first delta of a transition:

1. the current prepared target is active and has a current `Committed` tuple;
2. its committed view exactly equals the controller's displayed view and its
   committed model still matches its committed snapshot;
3. the finite pointer is inside the final visible logical rectangle; normalized
   `u,v` uses that tuple's final logical render rectangle and active physical render
   extent; and
4. any mechanical committed-view ray uses that same tuple and `u,v`.

Local geometry knowledge, packet-to-placement mapping and anchor selection stay
with the consumer. This boundary supplies no geometry-lease inspection, arbitrary
snapshot traversal, CPU mesh mirror, BVH, object selection or GPU ID query. A
nearest geometric hit need not survive alpha-mask discard or raster culling and
must not be represented as rendered-visible picking. No new anchor-source,
interaction binding, renderer capability or runtime admission is created here.
For an accepted Perspective anchor `A`, let its current camera-local coordinates
be `(x, y, z)`, with positive forward depth `d = -z`, and let `s = t' / t` be the
already checked half-angle-tangent scale from §9.3. The controller applies the
same FOV target plus the camera-plane translation:

```text
translation = camera_right * ((1 - s) * x)
            + camera_up    * ((1 - s) * y)
position'     = position + translation
orbit_target' = orbit_target + translation
```

The controller rejects an anchor at non-positive depth and validates that every
anchored wheel state preserves orientation, yaw, pitch, orbit distance, and
near/far, while position and orbit target move by one equal camera-plane
translation. It does not own ray construction, geometry, semantic selection, or a
Timeline handle. This is optical FOV zoom plus compensating pan, never physical
dolly.

An active transition retains its accepted world anchor (or `None`). Successive
fused deltas compose on the active target with that same anchor, then rebase the
80 ms Timeline from the displayed state. Perspective interpolation remains
geometric in `tan(vertical_fov / 2)` and derives the matching compensating-pan
fraction, keeping the accepted geometric point at its normalized coordinate at
intermediate samples as well as at the endpoint. Manual gesture entry,
projection/mode change, reset, suspension, unmount/destruction, and controller
failure discard the anchor together with the transition.

Any missing/inactive target, invalid pointer rectangle, absent committed tuple,
controller/committed-view mismatch, snapshot/model mismatch, ray failure,
unsupported consumer geometry mapping, non-finite triangle operation or geometric
miss selects the ordinary centre-only §9.3 target deterministically. Orthographic
wheel input always remains §9.4 pose-preserving span zoom and never uses this
anchor.

### 9.4 Orthographic Zoom

Orthographic zoom changes bounded positive vertical span while preserving pose,
near distance, and far distance.

Canonical routed scroll deltas are already additive logical pixels under
SPEC-HAL-033/036: line input is converted by InputFusion (currently a mouse line
uses a 56-pixel base before any configured fusion scale factor), pixel input
passes through, and bursts may be coalesced. The controller therefore applies no
line/pixel/device guess. It:

1. rejects non-finite input transactionally;
2. clamps the fused logical-pixel delta to the policy's finite per-event bound;
3. computes a multiplicative response with the policy's finite positive
   exponent per logical pixel; and
4. saturates Perspective half-angle tangent at its derived minimum or exact
   initial/default maximum, and orthographic span at either derived absolute bound.

The default exponent makes one current mouse line a modest multiplicative step,
while pixel deltas retain proportional touchpad precision. The default per-event
cap admits at most a small bounded multi-line burst. Consumer policy may change
both values, but their worst-case exponential product must remain finite and
non-zero when checked.

### 9.4A Smooth Wheel-Zoom Transition

The retained consumer may smooth wheel zoom through one optional existing
`Timeline` transition. The consumer owns only its transition handle, current
base state, and latest checked target. `CameraNavigationController3` remains
the sole owner of wheel response, camera geometry, projection semantics, bounds,
and immutable view construction.

For each accepted fused delta, the consumer asks the controller for a checked
target from the active transition target, or from the current state when none is
active. It cancels the prior handle, starts an 80 ms `EaseOut` transition from the
currently displayed state to the composed target, and publishes at most one
checked interpolated camera state per timeline sample. Completion publishes the
exact target. A missing public animation handle applies that already-checked
target immediately. A new delta rebases from the current displayed state; it
does not restart from an obsolete base, reduce a burst to a sign, or queue
intermediate wheel states.

The controller exposes only a narrow checked boundary: compute a wheel target
from its current state or a prior valid target, interpolate a compatible
wheel-only pair, and apply that compatible interpolated state without clearing
held keys or pointer gesture state. `reset` remains the only state replacement
operation that clears transient input.

Manual camera gesture entry, projection or mode change, reset, custom-render
suspension, consumer unmount/destruction, and controller failure discard the
pending transition before their normal operation. They retain the currently
displayed checked state and must never apply an obsolete target later. Geometry
does not define wheel-zoom semantics, but a consumer must similarly discard a
transition whenever its presentation becomes unavailable.

When an owner has an `EventContext`, it cancels the Timeline handle and clears
the complete transition record in that same routed callback. When a control
change or custom-render preparation observes cancellation outside an event
callback, the consumer retains a pending cancellation request and its optional
handle until its next `UpdateContext`; that update cancels the handle and clears
base, target, and any retained anchor before calling `advance`, sampling the
Timeline, or issuing a later camera request. Clearing an `Option` alone is not
cancellation and may not orphan a running Timeline.

Except for the existing checked §9.3A anchored target operation, this transition
adds no cursor anchor. Perspective otherwise remains centre-only optical FOV zoom
and orthographic remains pose-preserving span zoom. It adds no off-axis frustum,
generic screen-to-world, generic raycast or generic picking contract; the 2D
affine cursor-anchor rule is not reused here.

### 9.5 Fly Translation

`FlyMotionOperation3::{Forward, Backward, StrafeLeft, StrafeRight}` combines its
held axes into the checked target velocity described in §8. The controller's
bounded velocity response translates position and target together. It is inactive
in Orbit mode. `OrbitModifier` resolves the Fly secondary-drag operation and
contributes no target velocity. `SpeedModifier` changes only the checked target
speed multiplier while held. This is camera translation, not physical dolly and
not a model transform.

### 9.6 Reset

Reset is an explicit consumer-provided checked `CameraState3`. It clears transient
input and atomically replaces camera presentation state without touching model
state.

---

## 10. Public API Boundary

The public navigation module provides the following conceptual units:

```text
CameraNavigationMode3
CameraPointerBindings3
CameraNavigationPolicyDescriptor3
CameraNavigationPolicy3
FlyKeyBindings3
FlyMotionOperation3
CameraPointerOperation3
CameraWheelOperation3
CameraState3
CameraNavigationController3
CameraNavigationError
```

The checked policy type is constructed from a descriptor. The controller accepts
canonical `KeyEvent` plus `KeyboardAuthorityMode`, policy-resolved pointer
gesture operations, canonical logical-pixel scroll, explicit
mode/projection/reset operations, and frame `dt`.

The controller exposes read-only state, mode, active-motion state, and immutable
`ViewDescriptor3` output. It does not depend on `Viewport3D`, `ModelState`,
`RenderSnapshot`, `EventContext`, a renderer, GPU resources, Timeline, or an OS
window type.

Framework requests such as focus, capture, invalidation, and continuous frames
remain in the consuming widget because only the widget's routed `EventContext`
and `UpdateContext` have that authority.

Cursor visibility remains outside the pure controller API. The consumer uses
`EventContext::request_cursor`; EventRouter binds hidden persistence to the
captured widget and the platform layer applies visibility. No controller method,
OS handle, or `Viewport3D` cursor API is added.

For §9.4A, the controller additionally exposes checked wheel-target,
wheel-interpolation, and wheel-transition-application operations. They accept
only the controller's compatible camera/projection family and may change only
the perspective vertical FOV or orthographic span. A centre-only perspective pair
preserves position, orientation, yaw, pitch, orbit target, orbit distance, and
near/far exactly. The separate anchored target operation accepts one explicit
world point and may move position and orbit target only by one equal
camera-plane translation; interpolation is geometric in `tan(vertical_fov / 2)`
with the matching translation fraction. These operations do not accept a Timeline
handle or acquire lifecycle authority.

---

## 11. Integration Authority Boundary

The generic integration contract remains §§6–10: routed focus/capture and cursor
lifecycle, checked camera-only operations, immutable view publication and
consumer-owned Timeline cancellation. Application controls, initial mode,
diagnostics, scene content, figure selectors and model-transform bindings remain
consumer-owned; this SPEC does not prescribe them or infer a transform target.

The previous consumer-specific requirements in this section are superseded.
Their exact text and related evidence are preserved in the private historical
development record dated 2026-10-27. That record supplies no framework
authority or implementation admission.

---

## 12. Normative Invariants

**INV-356-OWNER:** Camera state and navigation policy are owned by the retained
application/widget controller, never `Viewport3D`, `Context3D`, renderer, or OS.

**INV-356-CAMERA-ONLY:** Every navigation operation mutates only camera state and
transient controller state. Camera-only input leaves `ModelState`, render snapshot,
geometry, and placements byte-for-byte/identity unchanged.

**INV-356-OPERATION-TARGET:** Every admitted camera binding resolves to one
named camera operation and the `CameraState3` target owned by its checked
policy. An unclaimed binding produces no camera operation. No camera operation
may proxy a transform interaction or infer a target from visual output.

**INV-356-IMMUTABLE-VIEW:** Rendering receives only an immutable checked
`ViewDescriptor3` under a fresh view revision when camera presentation changes.

**INV-356-CAPTURE:** Character bindings mutate camera state only when routed to
the focused controller consumer in keyboard `Capture` mode.

**INV-356-CONCURRENT:** Default Fly camera navigation leaves primary pointer
input unclaimed. Held-key integration may continue while ordinary scene pointer
interaction is routed, and camera pointer capture exists only for the resolved
secondary-look or auxiliary-pan gesture lifetime.

**INV-356-FLY-TETHER:** Default Fly resolves Space held in keyboard Capture at
secondary down to `OrbitRotate` around only the current explicit
`CameraState3::orbit_target` and retained distance. Space alone and `X` are not
vertical camera-translation bindings. The accepted pointer operation remains
fixed through that capture lifetime; no selector, wheel anchor, ray, geometry,
or rendered output supplies a tether pivot.


**INV-356-CURSOR-OWNER:** A hidden cursor is owned by exactly the retained layer
that owns the active captured drag. Default Fly camera look may own the
secondary-button lease; a scene/object drag may separately own a primary-button
lease. Neither layer may hide, restore, or prolong the other's cursor state.

**INV-356-CURSOR-LIFETIME:** The default Fly look cursor is hidden only while the
same widget owns its active secondary-button capture. Every normal, error,
capture, focus, destruction, window-loss, and shutdown exit in §6.3
restores cursor visibility before hidden state can outlive the capture owner.

**INV-356-RELEASE:** Capture leave, focus loss, unmount, and explicit cancel clear
all held keys, retained Fly velocity, and pointer gesture state before another
update can move the camera.

**INV-356-FLY-ENTRY:** An explicitly requested entry through the existing routed
focus/capture path is not complete until the mounted viewport receives its actual
`on_capture_enter` callback. A pending entry request is not capture authority and
must not recreate Capture after Escape, blur or an ordinary capture leave.

**INV-356-MODE-TEARDOWN:** A mode transition uses the existing focus/capture
lifecycle before admitting the resulting mode's keyboard authority. It cannot
leave an old pointer capture, hidden cursor, transient scene gesture or pending
wheel Timeline active under the new mode.

**INV-356-WHEEL-TEARDOWN:** A pending wheel Timeline keeps its owner-addressable
handle until a callback or update cancels it. Custom-render inactivity clears
the entire pending wheel record before a subsequent camera sample or request.

**INV-356-TIME:** Held motion uses only engine-supplied update `dt`; key repeat,
wall clock, Timeline, and private schedulers contribute zero movement authority.

**INV-356-CADENCE:** For admitted `dt` values within policy bounds, equal total
elapsed time and equal held state produce equal displacement within the declared
floating-point tolerance, independent of update cadence.

**INV-356-BOUNDED:** State, policy, pointer/scroll input, `dt`, orientation, orbit
distance, orthographic span, speed, and resulting pose remain finite and within
declared bounds. Failure is transactional.

**INV-356-SCROLL-UNITS:** Optical/span zoom sensitivity consumes canonical additive
logical-pixel scroll deltas. InputFusion owns line/pixel normalization; the
camera controller owns finite per-event admission, multiplicative response, and
deterministic perspective-tangent/span saturation.

**INV-356-PERSPECTIVE-DEFAULT-MAX:** The checked initial/default Perspective
half-angle tangent is the exact maximum FOV endpoint for its controller lifetime.
Wheel zoom-in may narrow only to the derived minimum; wheel zoom-out may restore
only to that default and never produces a wider Perspective FOV. Orthographic
span retains independent finite minimum and maximum endpoints.

**INV-356-WHEEL-TRANSITION:** A smooth wheel transition is consumer-owned
Timeline interpolation over controller-checked wheel-only states. New fused
input composes onto the latest target and rebases from the displayed state;
manual interaction, state replacement, unavailable presentation, teardown, or
failure discard an obsolete target. Timeline never owns input, bounds, camera
state, or held motion.

**INV-356-ANCHORED-TARGET:** The checked anchored operation consumes an explicit
world point under §9.3A's currentness and finite-state boundaries. It supplies no
anchor source or rendered-visible picking. Invalid/stale presentation data or a
consumer geometric miss falls back to centre-only Perspective FOV zoom.

**INV-356-MODE:** Mode switching clears transient input and preserves camera pose
and projection until subsequent explicit input.

**INV-356-PAN:** Pan and Fly strafe translate camera presentation state; they do
not simulate camera movement by modifying world/model transforms.

**INV-356-ROUTING:** Navigation consumes only canonical routed events. No direct
platform or global input path exists.

**INV-356-ISOLATION:** A default application with no camera controller and every
ordinary 2D widget has identical input routing, update scheduling, layout, paint,
and rendering behavior.

---

## 13. Forbidden Behaviors

**FORBID-356-RENDERER-CONTROLLER:** No camera state, key map, gesture state,
orbit/fly policy, or held-key integration inside `Viewport3D`, `Context3D`, the
custom-render participant, renderer, or GPU resource owner.

**FORBID-356-MODEL-MUTATION:** No camera operation may mutate `ModelState`,
`RenderSnapshot`, geometry, model placement, or application world data.

**FORBID-356-TRANSFORM-PROXY:** No camera binding may be named or implemented
as `rotate_scene`, model pan, model dolly, or another transform interaction;
camera/dolly-looking visual output does not establish a transform target.

**FORBID-356-TRANSFORM-INFERENCE:** Camera navigation must not resolve an
application transform target from packet ordinal/slot, model order, geometry,
appearance, wheel anchor, ray hit or rendered visibility. A consumer's target,
pivot, space, bindings and calibration remain outside the camera controller and
do not acquire generic selection, picking, hierarchy or transform authority here.

**FORBID-356-SECOND-CLOCK:** No controller-owned `Instant`, wall clock, timer,
worker cadence, frame counter, or scheduler for navigation motion.

**FORBID-356-REPEAT-MOTION:** No distance or rotation derived from key-repeat
count or repeat cadence.

**FORBID-356-IMPLICIT-CAPTURE:** Focus alone, hover, scroll, construction, mode
selection, or a global key must not silently enter keyboard capture.

**FORBID-356-NAVIGATION-LEAKAGE:** No camera mutation from Navigation mode,
unfocused routing, text input, unbound keys, stale held state, or another window's
interaction domain.

**FORBID-356-FLY-POINTER-MONOPOLY:** Fly selection, keyboard capture, or held
motion must not claim primary pointer input, require Fly to be disabled for scene
interaction, or retain look pointer capture after the bound button is released.

**FORBID-356-CURSOR-LEAK:** No hover, focus, held key, unclaimed input, failed
capture, inactive scene path, ended gesture, lost focus, cancelled interaction,
unmount, destruction, or window shutdown may leave the cursor hidden. Camera
navigation may not acquire or release a scene/object layer's primary-drag cursor
ownership.

**FORBID-356-PLATFORM-SHORTCUT:** No direct `winit`, OS, raw device-event, cursor,
or window call from the controller to bypass canonical ingress or engine capture.

**FORBID-356-FALSE-RELATIVE-POINTER:** No pointer-lock, relative-mouse,
unbounded-look, or cross-platform physical-key claim without the corresponding
implemented and verified input authority.

**FORBID-356-UNNORMALIZED-WHEEL:** No treating canonical logical-pixel scroll as
lines, applying platform/device constants after InputFusion, or unbounded
exponential response to a coalesced or extreme delta.

**FORBID-356-UNBOUNDED-MOTION:** No unchecked non-finite input, unbounded pitch,
zero/negative distance or zoom span, unbounded stall `dt`, or arithmetic overflow
committed to camera state.

**FORBID-356-2D-CONTAMINATION:** No navigation branch, state, update request, or
input interception in ordinary 2D widgets, shared 2D spatial state, the 2D
renderer, or an application that did not instantiate the controller.

---

## 14. Conformance Proof

A conforming implementation includes executable proof for:

1. camera-only mutation leaves model/snapshot identity unchanged;
2. Orbit rotate, true pan, centre-only or checked compensating-pan perspective
   optical FOV zoom, and orthographic span zoom preserve finite checked state and
   existing view semantics;
3. Fly forward/back and strafe use normalized engine-`dt` target velocity, while
   its Space orbit modifier and speed modifier contribute no translation by
   themselves;
4. equal-duration 60 Hz and 120 Hz acceleration, steady motion, and release
   deceleration produce equal displacement within tolerance;
5. repeat events add no motion or velocity, and key-up decelerates to a finite
   stopped state;
6. capture leave/cancel/unmount lifecycle clears held, velocity, and gesture state;
7. Navigation mode and unbound/text input do not leak into camera state;
8. Orbit/Fly switching is pose-preserving and clears transient input;
9. default Fly maps ordinary secondary drag to free look, Space-secondary drag
   to tether orbit around the explicit camera orbit target, auxiliary drag to pan,
   and primary input to no camera gesture while held movement remains independently
   active;
10. default motion keys are W/S and A/D, Space is only the Fly tether modifier,
    no vertical key binding exists, and alternate checked pointer/key bindings
    remain configurable;
11. one current mouse-line equivalent produces a modest logical-pixel response;
12. repeated wheel input and extreme finite deltas saturate Perspective exactly at
    its finite minimum or initial/default maximum without crossing, while
    Orthographic independently saturates at both finite span bounds;
13. bounds and pose remain coherent through Orbit/Fly and projection transitions;
14. existing Orbit behavior remains available after Fly integration;
15. default Fly secondary look hides the cursor only after camera pointer capture
    is admitted and restores visibility on normal pointer up;
16. movement failure, explicit cancel, capture cancellation/denial, Escape/Tab
    capture leave, blur/focus loss, unmount/destruction,
    and window loss cannot leave a stale hidden cursor;
17. Fly primary input produces no camera cursor request, while an active
    scene/object primary drag retains independent ownership of the same lifecycle;
18. a routed consumer integration exercises both camera modes; and
19. no-controller and ordinary 2D paths remain structurally isolated.
20. smooth wheel targets compose from full fused deltas, interpolate to the same
    exact terminal camera state across display cadences, preserve perspective
    optical and orthographic pose semantics, and never clear held or
    pointer transient input during an intermediate application;
21. manual gesture entry, projection/mode change, reset, unavailable
    presentation, teardown, and failure discard a pending wheel target.
22. a scoped Showcase nearest-geometric anchor uses only active committed
    view/model/snapshot/target data, preserves its normalized Perspective point
    through checked FOV-plus-pan interpolation, and falls back to centre-only on
    every invalid/stale/miss path; and
23. alpha-mask and culling disagreement remains observable as a geometric-prototype
    limitation, not a rendered-visible-picking claim; and
24. every default camera pointer, key, and wheel binding resolves to its named
    camera operation and `CameraState3` target, while unclaimed Fly primary input
    remains available to a separately owned transform/selection policy.
25. active-to-inactive custom-render transition during a wheel Timeline cancels
    that exact Timeline and clears its anchor before any later update sample or
    camera request; reset/control callbacks never orphan a running handle;
26. Fly/Orbit changes during active camera look or an independently owned scene
    drag route lifecycle cancellation that releases pointer capture, restores the
    cursor, clears the gesture, and prevents a subsequent move from continuing
    it; and
27. where an integration uses explicit control-driven Capture entry, it proves
    actual routed `on_capture_enter` followed by routed key-down, update tick and
    key-up without a free-look prerequisite. Scheduling or changing a control's
    value alone is not acknowledgement.

Tests may use the pure controller directly for deterministic camera proofs.
Existing EventRouter tests remain authoritative for engine-owned focus/capture
routing; the camera vertical must additionally prove it invokes the required
consumer lifecycle hooks.

---

## 15. Portability Boundary

| Capability | Binding baseline |
|---|---|
| Routed absolute pointer drag | Required now |
| Engine pointer capture | Required now |
| Engine keyboard capture and Escape release | Required now |
| Fly primary-button scene coexistence | Required now |
| Capture-owner-bound cursor hide/restore | Required now for default Fly secondary look; available to an independently owned scene drag |
| Logical character bindings | Required now, configurable |
| Canonical logical-pixel scroll response | Required now, configurable and bounded |
| Engine-`dt` held motion | Required now |
| Relative raw mouse motion | Not implemented / not claimed |
| Cursor confinement or pointer lock | Not implemented / not claimed |
| Physical-layout key bindings | Not implemented / not claimed |
| Browser pointer lock | Not implemented / not claimed |
| Gamepad/touch fly policy | Not implemented / not claimed |

The absent rows do not block the portable captured-drag baseline and do not
authorize stand-alone negative proof work. They become work only under separate
owner authority admitting the intended capability.

---

## 16. Compatibility and Non-Goals

This SPEC preserves:

- SPEC-HAL-355 immutable rendering and `Viewport3D` mechanical ownership;
- existing Viewport3D consumers that supply `ViewDescriptor3` directly;
- existing Orbit rotation and true-pan behavior;
- bounded Perspective optical FOV and orthographic span zoom semantics, including
  only the existing §9.3A checked anchored target operation;
- camera operation/target naming that keeps physical dolly distinct and leaves
  transform-target resolution to the consumer;
- ordinary EventRouter focus/capture semantics;
- ordinary primary-button scene interaction while Fly navigation is active;
- Timeline as generic interpolation rather than input timing;
- SPEC-HAL-343's 2D affine spatial substrate without extending it into a
  perspective/3D transform framework; and
- default 2D input/update/render behavior.

Non-goals are physical wheel dolly, cursor anchoring or compensating camera pan
outside §9.3A's checked operation, off-axis frusta, rendered-visible picking,
generic scene picking, generic snapshot traversal, GPU ID picking, CPU mesh
mirrors, BVHs, collision, gravity, non-controller flight physics, target
following, gizmos, generic selection semantics, generic group/root membership,
generic transform translate/scale, camera persistence, key-rebinding UI,
gamepad/touch control, relative-pointer platform ingress and a generalized
multi-dimensional navigation framework. The bounded §8 controller velocity
response is the only admitted Fly acceleration/deceleration behavior. A consumer's
figure map, binding, local pivot/space, rotation calibration and example mesh are
not generic framework selection, transform or reusable-geometry contracts.
Broader framework capabilities require separately accepted owning authority.

Binding SPEC-HAL-357's one-level local-domain/content identity carrier is
not generic group/root membership, selection, picking or transform-binding
policy. It does not alter these non-goals or supply consumer transform authority.

---

## 17. Constitutional Consistency Matrix

| Pillar | Compliance |
|---|---|
| A — Retained-Mode Ownership | Controller state lives in one retained widget/application owner; no second tree or renderer ownership |
| B — Phase Separation | Input changes transient/camera state during routed event authority; held motion updates during `Widget::update`; layout/paint/render consume immutable state |
| C — ID-based Identity | Focus/capture routing remains WidgetId/domain based; controller stores no widget references |
| D — Pure Data / Explicit Actions | Checked camera/policy data in; explicit routed operations and immutable view out |
| E — GPU-First Rendering | Renderer receives a frozen checked view and no controller mutation during preparation/paint/submission |
| F — Engine Sovereignty | EventRouter owns focus/capture/routing and EngineTicker owns time; OS remains transport only |

This SPEC does not amend SPEC-HAL-002. It supplies a subordinate interaction
policy that preserves every permanent invariant.

---

## 18. Adoption Decision

Adoption creates binding authority for the optional camera navigation policy and
exactly these implementation units:

1. checked public camera state, policy, and pure controller operations;
2. existing-router capture integration in a consuming widget;
3. concurrent Fly keyboard motion, short-lived camera gestures, and unclaimed
   primary scene input;
4. bounded logical-pixel optical FOV/span zoom with scale-relative finite limits;
5. engine-`dt` Fly held motion without a second clock;
6. pose-preserving Orbit/Fly switching;
7. capture-owner-bound cursor hide/restore for default Fly secondary look without
   acquiring primary scene authority; and
8. the conformance proof in §14.

Adoption does not mark any implementation complete and does not alter
SPEC-HAL-355 rendering conformance. Implementation claims require live source and
executed tests against the claiming revision.

### 18.1 2026-08-28 Binding Adjacency

This documentation amendment records adopted SPEC-HAL-357 as a binding
adjacency and clarifies that domain placement remains outside camera state and
navigation target policy. Spatial co-presence, equal coordinates, an identity
embedding, overlap, or a committed domain ray supplies no camera pivot, semantic
relation, selection, or successor-state rule. Camera-only changes continue to
leave every domain composition/state/frame and consumer source state unchanged.

Any future `CommittedDomain3` check would establish only mechanical
target/view/domain currentness. Consumer semantic/representation revision
validation remains separate and must not enter `CameraNavigationController3`.
Likewise, binding SPEC-HAL-355 alone determines whether an active compatible
`FailedWithLastGood` exposes its old committed ray/association; navigation may
neither substitute requested metadata nor reinterpret that lifecycle.

It reciprocally reconciles SPEC-HAL-356 with SPEC-HAL-357 adoption and changes no
binding camera operation, Showcase behavior, implementation claim, or
conformance obligation in this SPEC.
