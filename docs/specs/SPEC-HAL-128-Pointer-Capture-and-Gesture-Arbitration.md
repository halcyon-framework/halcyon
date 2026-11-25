# SPEC-HAL-128

**Pointer Capture & Gesture Arbitration Contract**

**Status:** Implemented
**Scope:** Pointer capture, multi-gesture arbitration, drag/resize/scroll conflict resolution
**Applies to:** All interactive widgets using pointer input (Scrollbars, Sliders, ResizeHandles, Tree drag, Column resize, Canvas tools)
**Layer:** Input routing / Gesture state machine
**Depends on:**

* SPEC-HAL-127 (Overlay Stack & Input Capture)
* SPEC-HAL-110 (Unified Scroll Overlay & Viewport Contract)
* SPEC-HAL-121 (Tree Drag-and-Drop Reordering), where applicable

**Non-Goals:**

* Touch gesture recognition (pinch/rotate)
* Accessibility narration (SPEC-HAL-125)
* Animation curves or inertia
* Visual affordances

---

## 1. Problem Statement

Pointer-driven UIs fail when multiple gesture candidates compete without a **single arbiter**, causing:

* scroll vs drag conflicts
* resize jitter
* lost pointer-up events
* stuck interaction states
* inconsistent cancellation behavior

This SPEC defines a **global pointer capture model** and a **deterministic gesture arbitration protocol**.

Any widget bypassing this protocol is **incorrect by definition**.

---

## 2. Core Concepts

### 2.1 Pointer

A logical pointing device stream identified by `PointerId`.

* Each pointer has its own lifecycle.
* Mouse = one pointer.
* Touch/stylus may produce multiple pointers (out of scope for recognition, in scope for capture).

#### Native contact transport — Reader continuation (2026-10-21)

JC's Reader continuation admits native touch transport and HAL-112's scoped
Button activation through the existing platform adapter, InputFusion and
canonical fused EventRouter route. It does not adopt Draft HAL-079's recognizer
arena or change the Stage 7 gesture-legality matrix.

Raw touch carries native contact identity, window, timestamp, logical position
and Started/Moved/Ended/Cancelled phase. InputFusion maps each live
`(window, native contact)` to a checked nonzero logical `PointerId`; native ID
zero cannot alias Mouse. Logical IDs are not reused during that Fusion
lifetime, including across focus loss. Existing GestureRegistry remains the
sole physical-press/session allocator; its immutable press retains the actual
pointer ID/type through qualified normalization and routing.

Begin/move/end/cancel preserve order and exact contact identity. Contact
boundaries flush preceding desktop batches; contact motion is not coalesced
across identities. End/cancel retires only its contact. Focus loss cancels that
window's contacts before its focus terminal. Coordinate normalization occurs
once at the existing platform boundary using the effective target scale.

Native App suspension explicitly retires queued contact envelopes and cancels
live contact captures through the same tree-aware owner before stopping output.
It does not fabricate a focus event or replay a pre-suspension press on resume.
Contact cancellation is engine lifecycle work and cannot be swallowed by a
Screen or modal. Ordinary touch Down/Move/Up retain HAL-127 shielding; a contact
that dismisses an overlay cannot click through on its remaining sequence.

Touch Down/Move/Up share canonical absolute pointer routing and its four phases.
Widget capture, release and commit bookkeeping use the actual event pointer ID.
The legacy single-pointer capture mirror remains Mouse-only; a touch terminal
cannot release Mouse or another contact. Touch Cancel reuses tree-aware
PostProcess teardown without synthetic Up, activation or desktop hover change.

Existing widgets receive touch only through HAL-112's static participation
declaration. A native transport adapter never mutates focus/capture or invokes
widgets. No Reader-specific route or touch-to-Mouse translation is admitted.

### 2.1.2 Direct touch content scrolling

[SPEC-HAL-396](SPEC-HAL-396-Touch-Content-Scrolling.md) supplies the accepted direct touch-content-scroll profile: immutable qualified motion facts, gesture-only ScrollArea admission, exact recognition and pending priority, exclusive domain authority, and the audited transfer of an unfinished Button press. The same qualified contact route and existing owners remain canonical. HAL-396 explicitly supplements the existing Touch admission and priority rules for this profile; other gesture profiles retain their existing contracts.


---

### 2.2 Pointer Capture

A state where:

* a widget receives **all subsequent pointer events** for a given `PointerId`
* regardless of pointer position

Capture is **explicit**, scoped, and exclusive per pointer.

---

### 2.3 Gesture Candidate

A widget-local interaction that *may* become active, such as:

* drag
* resize
* scrollbar thumb drag
* canvas pan
* selection marquee

Candidates compete until one is committed.

---

## 3. Pointer Capture Invariants

### INV-PC1 — Explicit Capture Only

Pointer capture MUST be requested explicitly.

Implicit capture (based on hover or press alone) is FORBIDDEN.

---

### INV-PC2 — Per-Pointer Exclusivity

At most **one widget** may capture a given `PointerId`.

Different pointers MAY be captured by different widgets.

---

### INV-PC3 — Capture Lifetime

Capture begins:

* only after a gesture is committed

Capture ends:

* on pointer release
* on explicit cancellation
* on widget destruction

Capture MUST NOT survive beyond pointer lifetime.

---

## 4. Gesture Arbitration Model

### INV-GA1 — Candidate Registration on Pointer Down

On pointer down:

* widgets MAY register gesture candidates
* no capture occurs yet

Registration is local and non-exclusive.

---

### INV-GA2 — Movement Threshold Commit

A gesture becomes active only when:

* pointer movement exceeds configured threshold
* candidate declares intent to commit

Before commit:

* no side effects allowed
* no capture allowed

---

### INV-GA3 — Single Winner Rule

At most **one** gesture may commit per pointer.

On commit:

* winning widget captures pointer
* all other candidates are cancelled

---

### INV-GA4 — Deterministic Priority

If multiple candidates request commit simultaneously:

Resolution order is:

1. Explicit resize handles
2. Explicit drag handles
3. Scrollbar thumb drags
4. Content drags
5. Scroll fallback

Priority MUST be fixed and documented.

---

## 5. Scroll Interaction Arbitration

### INV-GA5 — Scroll Is Last Resort

Scrolling is allowed only if:

* no other gesture commits
* scroll axis is valid and scrollable

Scroll MUST NOT preempt explicit drags or resizes.

---

### INV-GA6 — Axis Lock on Commit

Once a gesture commits:

* movement is locked to its axis (if applicable)
* perpendicular movement is ignored

No axis switching allowed mid-gesture.

---

## 6. Cancellation Semantics

### INV-PC4 — Escape Cancels Active Gesture

If Escape is pressed during active gesture:

* gesture is cancelled
* pointer capture is released
* state is restored exactly

---

### INV-PC5 — Lossless Cancellation

On cancellation:

* no partial state changes persist
* no application activation or commitment events are emitted
* visual indicators are cleared

A cancelled absolute `DragEnd` is terminal cancellation, not ordinary release.
It MUST NOT resolve pending commitment, synthesize `PointerUp`, or deliver a drop.
During PostProcess, the existing unified tree-aware cancellation owner MUST drain
pending requests, notify each remaining live candidate through
`on_gesture_cancel`, revoke capture and clear the gesture session, and notify the
former live capture owner through `on_pointer_capture_cancel` after revocation.

A widget holding both candidate and capture-owner roles may receive one
notification for each role. These are distinct framework lifecycle notifications,
not application events. Rollback and preview cleanup MUST be idempotent and MUST
NOT emit application commitment or activation, recapture, or change independent
focus/keyboard authority. An already-cleared session or repeated terminal produces
no new role notification. Destroyed `TreeWidgetId`s MUST NOT be invoked.

If a lifecycle callback changes state consumed by paint, its owner MUST emit the
narrowest paint invalidation unless the caller already guarantees it before
retained reuse (SPEC-HAL-354, SPEC-PM-018). Scheduling frames is not invalidation.
No layout invalidation or broad repaint is authorized by pointer cancellation.

### Geometry-dependent Absolute cancellation (2026-10-08)

Under §14's explicit opt-in, an admitted causal geometry operation or changed
dependency basis is terminal `SystemCancel` before subsequent semantic input or
the first affected scene. The exact press is retired before the existing
tree-aware cancellation owner drains pending commits, notifies live candidates,
revokes capture and notifies the former live capture owner. The two role
notifications remain distinct and rollback remains idempotent. A losing
candidate's dependency watch retires with arbitration and cannot cancel another
winner. No callback runs in Layout, Transition, Scene or Paint.

---

## 7. Pointer-Up Handling

### INV-PC6 — Guaranteed Pointer-Up Delivery

On ordinary release, if a widget captures a pointer:

* it MUST receive the corresponding pointer-up
* even if pointer leaves window bounds

Lost pointer-up is FORBIDDEN.

A cancelled interaction instead receives the terminal cancellation notifications
defined in §6, without a synthetic Up. A later physical Up MUST NOT commit a
cancelled handle's cleared local interaction.

A protected normal `DragEnd` delivers its one ordinary release; its trailing
physical Up only completes that press's bookkeeping. Retired/completed protected
envelopes cannot enter AppRoot, arbitrate, activate or tear down a newer session,
including after tree/registry replacement. Closing an unenrolled consumed Down's
geometry eligibility does not suppress its ordinary application's later Up.
Physical click history and Down/DoubleClick/TripleClick classification remain.

---

## 8. Overlay Interaction

### INV-PC7 — Overlay Supremacy

If an overlay captures input (SPEC-HAL-127):

* underlying widgets MUST NOT receive pointer events
* gesture arbitration happens **within** overlay only

No cross-layer arbitration allowed.

---

## 9. Responsibility Boundaries

### Input System Owns:

* pointer lifecycle
* capture bookkeeping
* arbitration enforcement
* event delivery guarantees

### Widgets Own:

* candidate declaration
* commit intent
* gesture state machine

### Widgets MUST NOT:

* capture pointer on pointer-down
* steal capture mid-gesture
* synthesize pointer events
* infer intent from velocity alone

HAL-101 section 2.5's internal child operations are not new pointer events, gesture candidates, commits or terminals. Retain original pointer/session provenance, actual host pointer authority, text-region gating, guaranteed ordinary Up and single cancellation. Child selection geometry is real child-local geometry. No host/child stage changes winners, delivers a duplicate terminal or routes around the one arbiter.

---

## 10. Forbidden Behavior

Implementations MUST NOT:

* allow multiple active gestures per pointer
* switch capture between widgets mid-gesture
* scroll while resizing
* resize while dragging
* commit gestures on hover
* ignore Escape cancellation

---

## 11. Test Requirements

Implementations MUST test:

* single-gesture commitment
* priority resolution
* scroll vs drag conflicts
* pointer-up delivery outside bounds
* Escape cancellation
* overlay interaction isolation

---

## 12. Reference Implementation

Planned components:

* `PointerRouter`
* `PointerCaptureManager`
* `GestureArbiter`
* `GestureCandidate`

---

---

## 13. Stage 7 — Pointer Type & Gesture Legality

### INV-PT1 — PointerType is Session-Scoped

Every gesture session has exactly one `PointerType`.

Assigned at `begin_session`.

Immutable for session lifetime.

---

### INV-PT2 — Legality is Static

Gesture legality = (`PointerType`, `GestureKind`, optional `GestureAxis`).

No widget state, focus, capture, or runtime context involved.

---

### INV-PT3 — Reject on Registration

Illegal gesture candidates are rejected at registration time.

No panics.

No side effects.

Session remains valid.

---

### INV-PT4 — Arbiter Sees Only Legal Candidates

Arbiter assumes all candidates are legal.

No legality checks inside arbitration.

---

### INV-PT5 — Mouse Backward Compatibility

`PointerType::Mouse` behavior is unchanged.

`GestureKind::Custom` always allowed.

---

### INV-PT6 — No Defaults

PointerType must be explicitly provided or derived.

Silent defaulting is forbidden.

---

### Stage 7 Legality Matrix

| PointerType | Drag | Scroll | Resize | Select | Pinch | Swipe | Draw | Custom |
|-------------|------|--------|--------|--------|-------|-------|------|--------|
| Mouse       | ✓    | ✓      | ✓      | ✓      | ✗     | ✗     | ✗    | ✓      |
| Touch       | ✓    | ✓      | ✗      | ✗      | ✓     | ✓     | ✗    | ✗      |
| Pen         | ✓    | ✗      | ✗      | ✓      | ✗     | ✗     | ✓    | ✗      |

Legend:
- ✓ = Allowed
- ✗ = Disallowed

---

## 14. Stage 8 — Declarative Gesture Surfaces (Widget Contract Layer)

### External Absolute Drag consumer contract

An external ordinary widget may implement `Widget` together with the publicly
exported `WidgetDowncast` support trait and be owned by `WidgetTree`. It declares
`Drag` through a static `gesture_surface`; the default Absolute
`pointer_motion_policy` applies. This admits no ListView internal extension or
alternate input route.

For this handle profile, primary Down records only tentative local input facts
and the pre-gesture value. It MUST NOT capture, begin a visible drag preview,
change application state, or decide a movement threshold. InputFusion owns
threshold recognition. The handle requests its already-registered candidate's
commitment on framework `PointerEventKind::DragStart`. The request is not
commitment.

The framework routes this DragStart using the original Down position and resolves
requests after routing. When uncaptured, it re-hit-tests that original position;
this is NOT a retained target-ID guarantee across intervening relayout. A consumer
relying on the original handle hit keeps its geometry stable until threshold
crossing. Only `on_gesture_commit` authorizes a visible preview; pointer capture is
then already framework-owned. Subsequent captured Move events carry router-produced
local logical coordinates, including outside the handle. The widget MUST NOT
query candidates, inspect pointer-device legality, implement a threshold, or
capture directly to obtain those updates.

Positioned child geometry is assigned during layout through
`LayoutContext::layout_child` and `LayoutContext::set_child_rect` under
SPEC-HAL-141. Gesture callbacks change only owned tentative, preview, or semantic
values; they MUST NOT write node rects. Product placement identity, authored
revision and interpretation remain consumer-owned and enter later legal
deferred/layout work. SPEC-HAL-024 governs event-local coordinates: no second
origin subtraction, DPR conversion, or manual scroll compensation is allowed.

### Opt-in geometry dependency and qualified physical press

**Author:** JC
**Decision:** Accepted on 2026-10-08 under JC's standing delegation, adopting
the reviewed proposal with its explicit qualifications and corrections.
**Implementation:** Implemented in 0.208.0-beta; independent source review accepted
2026-10-08. R4 closes current owner-target proof: Grip refresh/check passed;
native lib geometry 4/4, stage4 geometry 4/4, stage4 positioned 4/4 and lib focus
24/24 all ended normally. Native/product integration remain separate.

#### Pre-Flight

- Objective: Implement exact opt-in Absolute-drag geometry retirement through real ingress and the first affected frame.
- Mandatory startup and authority entry chain completed: Live shared triplet/PROJECT-ENTRY and retained full 002/002A; affected 128/354/012/026/141/145, domain/coordinate/frame owners and PM-000/002/003/004/009/018 refreshed against source before mutation.
- Governing authority and status: 002 Foundational, 002A Permanent, 128 Implemented, 354/012/141/145 Binding and 026 Final; this delegated decision admits only the following scoped addition.
- Authority change, if any: This subsection and §§6/7; scoped terminal-fact clauses in 354/012 and reference-only qualifications in 026/141/145. No Draft/Proposed promotion.
- Target layer and existing state verified: Original Halcyon master 8d0dd32fd8977b2aaa66e1ecd4138c23255d2672, 0.207.0-beta; empty index; only independent HAL-239/365 dirt before this unit.
- Owned mechanisms and files: Runtime widget/mod.rs, event/mod.rs, input_fusion.rs, event_router.rs, widget_tree.rs, app.rs, ScrollArea, List and ListViewBody; the six named owner documents; stage4 and retained_value_updates proofs; PROJECT-ENTRY, manifest, own lock version and changelog. Exactly 21 paths.
- Named invariants: Original press lineage; reserve is not install; exact mount/session lifetime; causal complete dependency domain; terminal before input/paint; coherent final capture appearance; one Update/Layout sequence.
- Forbidden behavior: Drain-time session stamping, pointer tombstones, alternate allocator/router/normalizer, dirty-root heuristics, late callbacks, second Update/repair frame, widget cross-reads, synthesized input or click-history reset.
- Existing mechanisms inspected and reuse decision: Existing registry allocator, Fusion normalization/coalescing, tree invalidation, router cancellation, App ingress/frame sequence, pending scroll/reveal/transition and immutable appearance publication.
- Relevant external or dependent boundaries: no application mutation/adoption. Preview/order/XY/accounting remain; HAL-239/365 and 236/363/364 remain inactive.
- Test ownership and placement: Three public stage4 tests and three internal retained_value_updates tests declared in PROJECT-ENTRY; preserve all existing proofs and assertions.
- Exact runnable verification commands and expected proof: Sequential Grip commands `verify package halcyon test geometry_dependent_absolute_ check timeout 600`, `verify package halcyon test positioned_consumer_absolute timeout 600`, and `verify package halcyon test focus_appearance_ timeout 600`. Six new executed proofs plus actual compatibility counts; no native/GPU campaign.
- Output or artifact completeness boundary: Complete production-qualified 21-path candidate, raw receipts and exact pins, unstaged/uncommitted for independent review.
- SemVer and changelog impact: Additive 0.208.0-beta; own manifest/lock version only and substantive newest Unreleased entry.
- Git, history-rewrite, destructive, and external effects: Beforeimages/index outside source; no stage/commit/ref/push/reset/stash/worktree/permission/native/application effects; preserve foreign bytes.
- Out of scope: Product dragging/import/training, renderer/scene records, Open/timing and general layout/input redesign.

#### Halcyon Pre-Flight

- Governing SPEC and read status: 002/002A and the exact affected 128/354/012/026/141/145 clauses and PM constraints read; this decision is subordinate to them.
- Widget policy or N/A: Only a static primary Absolute Drag handle opts in; ordinary controls retain their policies.
- Interaction added and policy authority: Existing HAL-128 SystemCancel ends an invalid admitted drag; no new gesture or threshold.
- Applicable FORBID rules: No cross-widget references, OS semantics, capture on press, phase leakage, broad invalidation or stale retained output.
- Existing framework mechanism inspected: Registry sessions/candidates, canonical geometry/scroll, tree-aware teardown, actual App checkpoints and current appearance facts.
- Reinvention risk: Extend the existing owners with owned optional metadata; no second gesture/input/frame system.
- Demo/example compliance: No demo/native action; synthetic proof is engine evidence only.

`AbsoluteDragGeometryDependency { viewport_owner: NodeId, reflow_root: NodeId }`
and default-None `Widget::absolute_drag_geometry_dependency` admit only a static
primary Drag/Absolute surface. The declaring anchor and both ancestors resolve
to current mounts; reflow_root encloses viewport_owner and the anchor. Freeze the
declaration and complete allocation/clip path for the physical press. Invalid
ancestry/profile is a programming-admission failure. The domain is explicitly
conservative and complete, including outer allocation owners where sibling work
can move the workspace. It is not an inferred layout-isolation boundary.

`SessionQualifiedInput` is an owned sealed event/press envelope with read-only
`event()` and Clone of the same immutable payload and record. Only the existing
GestureRegistry allocator reserves physical identity, with checked non-reuse;
reservation does not install/clear the routed session. The record includes
intrinsic registry/tree lifetime seals, domain/window/pointer/button, original
press facts/order and the tree's causal cutoff captured at normalization. No
public physical constructor exists. Synthetic fused-agent wrapping is explicitly
unqualified. Existing legacy raw/relative/frame/flush/router calls remain
source-compatible but cannot silently admit the opt-in profile or strip a
qualified pending press.

The qualified raw entry reads its actual tree's ephemeral causal evidence and
the existing relative gate; qualified flush and process_frame share the same
normalization/coalescing/emission implementation. Both App loops retain the
envelope through window/domain gates, AppRoot/modal/engine consumption, the
existing relative-facts gate and fallback. Different press seals never coalesce.
Ordered routing installs the reserved ID. Consumed/rejected unenrolled presses
close geometry eligibility without candidates/capture/watches. Protected
Double/TripleClick admission uses the real physical press without synthetic Down
or changing legacy multiclick policy. Old externally retained records remain
intrinsically stale; no permanent pointer-only tombstone is permitted.

WidgetTree records checked causal origins before an already-dirty return or
propagation. Own-origin and descendant-origin summaries remain separate and
bounded by live mounts. Structural changes record old/new allocation paths before
unlink; ephemeral layout take/restore and geometry output writes are not causes.
At admission compare the frozen complete dependency domain and allocation/clip
ancestors with the raw cutoff, closing queued Down followed by an affecting
mutation. Active watches are bounded by current candidates/sessions and retire
with loser/terminal/removal. No idle whole-tree geometry polling is authorized.

Invalidity evidence includes changed admitted root allocation/normalization,
causal reflow/structure/clip operations, actually changed presented scroll,
genuinely admitted pending scroll/restore/dirty-anchor/layout-anchor effects,
non-consuming direct request presence, a pending focus reveal that can move its
actual nearest ScrollArea, and actual affected active transitions. Rejected or
duplicate effects, clean anchors, unchanged targets, no-op automatic reveal and
paint-only preview work are not effects. Later numeric anchor/reveal/transition
resolution remains legal at its existing owner; pending admitted effects permit
conservative retirement before Layout without predicting future rectangles.

The router's one exact-session retirement checks registry/domain/session/mount,
retires the seal first, and reuses complete tree-aware cancellation. App runs it
before protected input reaches AppRoot, after application/engine mutation before
fallback/arbitration, after deferred/rebuild/removal, after actual pre-routing
scroll advance before its projection guard, and at the single Update-terminal
checkpoint before Layout. A stale watch only disappears. Post-layout anchor,
reveal, transition, realization and scene-suppression algorithms are unchanged.

`build_scene_with_gesture_sessions` shares the production retained preparation
and continuation with transient mutable domain/router/Fusion capability. Legacy
scene entry cannot paint an active protected session without those checkpoints.
After the one normal Update, complete retirement, settle only actually changed
live opted-in capture owners via
`settle_absolute_drag_geometry_appearance(FrameworkAppearanceFacts)`, and publish
final real facts once. This hook is synchronous fact-only/paint-only cache
settlement: no animation/timer, metric, product event, context retention or second
Update. Derive remaining drag/capture presentation from final state; no borrowed
drag or precomputed capture boolean spans retirement. Unchanged owners retain
their normal Update facts. Cancellation invalidates only changed preview/cache
paint before retained reuse and adds no layout or repair frame.

#### Candidate executor evidence — 2026-10-08

Production package check passed with 0 errors and 79 warnings. The six named
geometry tests all executed and passed: three in the public stage4 owner and
three in the retained-frame owner. The final focused test invocation nevertheless
ended with exit 124 after Grip reported a 300-second test timeout, despite the
requested `timeout 600`; that whole command is failed, not green. A preceding
combined check/test invocation likewise passed check and all six test bodies but
timed out. The separate positioned-consumer command passed 4/4, and the separate
focus-appearance command passed 24/24 (ten retained-owner, seven Slider and seven
RangeSlider tests). No further green command was repeated.

Earlier owned compile failures and the dead-mount cleanup assertion failure
remain historical failed receipts, followed by their causal corrections. Exact
argv, environment, source pins, exit status, duration and raw output are retained
outside the repository. Candidate source review and successful whole-command
geometry verification remain open; these results do not admit application adoption or
native behavior.

#### R3 conformance correction and retained timeout evidence — 2026-10-08

The R3 independent review rejected the submitted P2 candidate pending ordinary
capture preservation and complete watched clip evidence. The correction keeps
a live mounted capture and its initiating session when another button's
qualified Down arrives; that event retains its own reservation and follows the
existing captured-target route without installing a replacement session. Dead
capture validation and uncaptured session replacement retain their existing
owners. A future raw release observation is not semantic terminal delivery.

The existing frozen geometry watch now carries ordered, mount-qualified,
applicable ancestor viewport and widget clip declarations, including rounded
corners only where a widget clip exists. Input and Update-terminal currentness
compare those owned values without computing new coordinates or mutating
geometry. Unchanged clips and ordinary paint-only output remain valid.

The public owner adds
`geometry_dependent_absolute_qualified_secondary_down_preserves_ordinary_capture`;
the retained-frame owner adds
`geometry_dependent_absolute_changed_clip_retires_before_first_scene`, including
unchanged-clip paint and separate bounds-only/rounded-only Update changes. All
six original geometry bodies and historical assertions are preserved. R3 source
and these two additions have not been compiled or executed; acceptance and
successful whole-command geometry proof remain open.

The retained final test request selected `geometry_dependent_absolute_` with
`timeout 600`. Its outer process exited 1 after 300.079893 seconds; Grip's test
action reported exit 124 and "cargo test timed out after 300s". Both selected
harnesses printed normal terminal summaries, not just individual passes: lib
3/3 in 0.45s and stage4 3/3 in 0.71s. The last emitted Cargo target was
`tests/theme_density_spacing.rs`. The preceding removal-correction invocation
passed check, printed lib 3/3 in 0.56s and stage4 3/3 in 0.75s, then likewise
reported a 300-second timeout; its last emitted target was
`tests/tier1_widget_tests.rs`. Neither output contains a Doc-tests entry. These
receipts establish neither the timeout setting's source nor a remaining live
process or hang. No new verification or runner polling was performed in R3.

#### R4 owner-target proof and RV1 acceptance — 2026-10-08

Independent review accepted the corrected 21-path implementation after complete
source review on 2026-10-08. R4 is implementation evidence, not a reviewer rerun.
The preceding P2 timeouts remain failed whole commands, and R3's
uncompiled/unexecuted disposition remains dated history. No source correction
or equivalent green rerun was made in R4/RV1.

Retained execution evidence includes, for each receipt prefix below, a
`.request.json` with exact requested/executed argv, environment and start;
`.stdout.txt`/`.stderr.txt` with raw output; and `.terminal.json` with end,
duration, exit and child termination. `final.json` and `review-report.md`
bind the named results and final source comparison.

| Action | Actual result | Whole-command exit | Receipt elapsed | Receipt prefix |
|---|---|---|---|---|
| Grip refresh/check | 0 errors / 79 warnings; refresh 0 files | 0 | 10.115s | `step1-check` |
| Native lib geometry | 4 passed / 0 failed / 0 ignored | 0 | 55.701s | `step2-lib-geometry` |
| Native stage4 geometry | 4 passed / 0 failed / 0 ignored | 0 | 5.516s | `step3-stage4-geometry` |
| Native stage4 positioned | 4 passed / 0 failed / 0 ignored | 0 | 2.146s | `step4-stage4-positioned` |
| Native lib focus | 24 passed / 0 failed / 0 ignored | 0 | 1.852s | `step5-lib-focus` |

All four native harnesses printed normal terminal summaries and all four Cargo
processes exited normally with code 0. They are the specifically admitted native
fallback, not Grip-ledger results. Each had the same 300-second hard bound through
the installed `gtimeout` owned-process-group mode; none emitted a timeout signal.
The Grip refresh/check likewise ended normally under its 300-second action bound.
The recorded launches comprised these package/target/filter operations:

| Operation | Recorded package and target selection |
|---|---|
| Grip refresh/check | `verify package halcyon refresh check timeout 300` |
| Native lib geometry | `cargo test --locked --offline -p halcyon --lib geometry_dependent_absolute_` |
| Native stage4 geometry | `cargo test --locked --offline -p halcyon --test gesture_cancellation_stage4 geometry_dependent_absolute_` |
| Native stage4 positioned | `cargo test --locked --offline -p halcyon --test gesture_cancellation_stage4 positioned_consumer_absolute` |
| Native lib focus | `cargo test --locked --offline -p halcyon --lib focus_appearance_` |

The recorded environment unset `CARGO_BUILD_TARGET` and `CARGO_BUILD_TARGET_DIR`,
selected the workspace's existing target directory with `CARGO_TARGET_DIR`, and
selected that workspace's `Cargo.toml`. Native requests named the Cargo operation;
execution added `gtimeout --signal=KILL --verbose 300s`. Exact executable and
workspace paths remain in the retained request records. Target selection compiled
the same production library and declared test owners, without package-wide harness
fan-out, extracted tests, changed flags or resolution/network access.

Native lib geometry (all passed):

- `tests::retained_value_updates::geometry_dependent_absolute_viewport_change_clears_first_retained_preview`
- `tests::retained_value_updates::geometry_dependent_absolute_changed_clip_retires_before_first_scene`
- `tests::retained_value_updates::geometry_dependent_absolute_paint_only_work_preserves_live_preview`
- `tests::retained_value_updates::geometry_dependent_absolute_update_invalidity_settles_before_first_scene`

Native stage4 geometry (all passed):

- `geometry_dependent_absolute_invalidity_beats_pending_commit`
- `geometry_dependent_absolute_qualified_secondary_down_preserves_ordinary_capture`
- `geometry_dependent_absolute_reflow_before_threshold_does_not_retarget`
- `geometry_dependent_absolute_retired_terminal_cannot_touch_replacement_session`

Native stage4 positioned (all passed):

- `positioned_consumer_absolute_cancel_after_commit_restores_and_invalidates`
- `positioned_consumer_absolute_drag_start_outside_handle_commits_and_releases_once`
- `positioned_consumer_absolute_cancel_before_commit_drains_pending_intent`
- `positioned_consumer_absolute_cancel_after_owner_removal_is_inert`

Native lib focus (all passed):

- `tests::retained_value_updates::focus_appearance_window_and_interaction_domain_suspend_without_inventing_restoration`
- `tests::retained_value_updates::focus_appearance_pointer_and_scroll_capture_have_independent_owners`
- `tests::retained_value_updates::focus_appearance_coherent_snapshot_preserves_optional_focus_domain`
- `tests::retained_value_updates::focus_appearance_mode_only_transition_invalidates_only_keyboard_owner`
- `tests::retained_value_updates::focus_appearance_focus_resolver_domain_transition_repaints_old_and_new_targets`
- `tests::retained_value_updates::focus_appearance_defensively_filters_removed_owner_before_authoritative_cleanup`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_preserves_a_distinct_configured_capture_width`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_is_visible_around_the_filled_thumb`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_does_not_paint_an_unowned_cue`
- `tests::slider_focus_appearance::slider_focus_appearance_radial_left_value_field_keeps_dial_available`
- `tests::slider_focus_appearance::slider_focus_appearance_does_not_paint_an_unowned_cue`
- `tests::slider_focus_appearance::slider_focus_appearance_preserves_a_distinct_configured_capture_width`
- `tests::slider_focus_appearance::slider_focus_appearance_track_outline_is_separate_from_the_thumb_fill`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_track_capture_differs_from_navigation`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_bar_capture_differs_from_navigation`
- `tests::slider_focus_appearance::slider_focus_appearance_capture_is_distinct_without_changing_geometry`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_outset_stays_inside_the_existing_parent_clip`
- `tests::slider_focus_appearance::slider_focus_appearance_excludes_owned_text_and_value_field`
- `tests::range_slider_focus_appearance::range_slider_focus_appearance_bounded_endpoints_keep_visible_distinct_cues`
- `tests::retained_value_updates::focus_appearance_layout_deferred_focus_preserves_capture_transfer_engine`
- `tests::retained_value_updates::focus_appearance_layout_deferred_focus_is_published_on_the_next_frame`
- `tests::retained_value_updates::focus_appearance_old_and_new_owners_repaint_without_callback_requests`
- `tests::retained_value_updates::focus_appearance_window_loss_delivers_lifecycle_before_authority_teardown`
- `tests::slider_focus_appearance::slider_focus_appearance_indicator_local_visibility_and_capture_survive_parent_clip`

The one safe-runner observation established no active Halcyon verification before
execution. The five commands ran once and sequentially. All 21 candidate hashes
matched R3 before and after; all 1348 tracked/nonignored file records, HEAD,
manifest/lock/changelog and index bytes remained unchanged. HAL-239/365 stayed
byte-exact and excluded. R4/RV1 accepts only this bounded framework implementation
and proof; broad-suite, GPU, native and application integration acceptance remain
separate.

### INV-GS1 — Widget Declaration is Static

A widget's gesture surface declaration MUST be determinable without runtime input state.

Declaration MUST NOT vary based on:

* current pointer position
* active modifiers
* focus state
* capture state
* PointerType

---

### INV-GS2 — Widget Declares, System Decides

Widgets declare which gesture kinds they support.

Widgets MUST NOT:

* inspect PointerType
* resolve conflicts between candidates
* override arbitration outcomes
* conditionally register based on other widgets' state

---

### INV-GS3 — Registration is the Only Widget Action

Widget participation in gesture arbitration is limited to:

* registering candidates on pointer-down
* declaring commit intent when threshold met
* receiving commit/cancel notifications

Widgets MUST NOT:

* query session state
* enumerate other candidates
* inspect arbiter internals
* modify registration after pointer-down

---

### INV-GS4 — No Pointer-Type Branching

Widget gesture behavior MUST NOT branch on PointerType.

Forbidden:

* "if Mouse then Drag else Scroll"
* "if Touch then enable Pinch"
* "if Pen then enable Draw"

Legality filtering is the system's responsibility (Stage 7).

---

### INV-GS5 — Gesture Surface is Immutable Per Layout

A widget's gesture surface MUST NOT change between layout passes.

Changes require:

* explicit widget reconfiguration
* full re-registration on next pointer-down

Mid-gesture surface changes are FORBIDDEN.

---

### INV-GS6 — No Side Effects Before Commit

Widget MUST NOT mutate application state during:

* candidate registration
* threshold detection
* arbitration

Side effects are permitted only after commit notification.

---

### INV-GS7 — Cancel Restores Exactly

On cancel notification, widget MUST:

* restore all state to pre-registration condition
* release all tentative resources
* emit no events

Partial rollback is FORBIDDEN.

---

### INV-GS8 — Widget Cannot Refuse Commit

If the system commits a gesture to a widget:

* widget MUST accept
* widget MUST NOT reject or defer
* widget MUST NOT delegate to another widget

Commit is final and non-negotiable.

---

### INV-GS9 — Widget Cannot Refuse Cancel

If the system cancels a widget's candidate:

* widget MUST accept
* widget MUST NOT retry
* widget MUST NOT escalate

Cancel is final and non-negotiable.

---

### INV-GS10 — No Cross-Widget Coordination

Widgets MUST NOT:

* communicate with other widgets about gestures
* share gesture state
* coordinate registration timing
* implement distributed arbitration

All coordination flows through the system.

---

### Stage 8 Forbidden Behavior Summary

Widgets MUST NOT:

* inspect PointerType
* branch on pointer device
* query other candidates
* override arbitration
* mutate state before commit
* refuse commit or cancel
* coordinate with other widgets
* change gesture surface mid-gesture
* register after pointer-down

---

## 15. Final Note

Pointer input is **not a stream of pixels**.
It is a **stateful negotiation**.

If gestures fight,
the system has no authority — and no future.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under coordinated architectural adoption, with JC's final ownership and veto retained. The retained adoption record identifies the exact three source bindings and adopted scope. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
