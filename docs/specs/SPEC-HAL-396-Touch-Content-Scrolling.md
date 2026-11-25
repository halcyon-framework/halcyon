# SPEC-HAL-396: Touch Content Scrolling

Status: Binding implementation contract.
Date: 2026-11-20.
Authority: JC's implementation mandate after the control audit and rework of the touch-scroll capture-transfer rule.
Owner: Halcyon core input and the existing ScrollArea.
Canonical scope: This document adopts the reviewed Button-to-ScrollArea transfer and supplies the complete direct touch-pan profile required to execute it.

## 0. Scope and verification

- Objective: Make the existing Halcyon ScrollArea move its content with a native touch contact through the canonical input pipeline, including a contact beginning on a Button.
- Consumer scenario: Finger scrolling in a Halcyon WASM application, preserving ordinary taps, keyboard activation, mouse input and independent contacts.
- Governing authority: HAL-002 and HAL-002A retain permanent authority. HAL-112 governs Button activation. HAL-128 governs qualified input, arbitration and capture. Locked HAL-131A retains scroll-substrate and exclusive-authority invariants. This new SPEC is the explicit supplemental touch profile required by its locked status; it does not unlock that document.
- Authority change: Adopt the audited unfinished-Button-press exception and the recognition, pending priority, nesting, first-contact, motion, cancellation and browser-canvas rules below. The historical proposal alone is no longer an execution gate or a second active owner.
- Source baseline at adoption: Halcyon 0.263.0-beta. Native Touch is normalized into separately identified qualified input. Existing DragStart/DragUpdate routing is Mouse-oriented. ScrollArea has scrollbar and wheel handling but no admitted direct touch-content pan. A captured Button prevents ordinary Move bubbling to its ancestors.
- Existing owners: InputFusion, the qualified input envelope, EventRouter, GestureRegistry, PointerCaptureManager, interaction-domain scroll authority, Widget policy, Button, ScrollArea, ScrollState, ScrollContainer and Halcyon's WASM window/canvas initialization.
- Named invariants: Immutable physical-press lineage; ordered delivery; one gesture winner and capture owner; one scroll authority per interaction domain; disarmed Button never re-arms; no false start; exactly-once displacement; current geometry survives cancellation; narrow retained-paint invalidation.
- Forbidden behavior: A second arena/router/contact allocator; fake Mouse/Wheel input; direct cross-widget mutation; full-session teardown to cancel the former Button; replacement of a winning gesture; overlay/domain escape; stale callbacks; unqualified shared touch state; browser/HTML scrolling substituted for Halcyon scrolling.
- Reuse: Extend the existing qualified transport, candidate registry, explicit capture, owner-local callbacks and scroll substrate. Draft HAL-079 is not adopted. No new gesture framework or feature flag is required.
- External boundaries: The library must compile for WASM. Browser ingress evidence and physical iPhone/Android results remain separately identified. Application deployment, backend and origin configuration remain outside this framework contract.
- Test owner: tests/gesture_cancellation_stage4.rs and existing InputFusion/platform inline tests; no duplicate harness. Section 9 names the acceptance traces.
- Verification commands: `cargo check --locked --offline -p halcyon`; `cargo test --locked --offline -p halcyon touch_scroll_`; `cargo test --locked --offline -p halcyon touch_contact_`; `cargo test --locked --offline -p halcyon positioned_consumer_absolute`; `cargo check --locked --offline -p halcyon --lib --target wasm32-unknown-unknown`. Report the actual selected tests, compilation target and terminal results.
- Completeness: Runtime source, routed acceptance evidence, native regressions and the WASM gate must be reported. Missing browser/device observation must stay explicit and must not be represented by mounted-widget results.
- Version/changelog impact: This adds supported touch-content scrolling. Apply the repository's additive-feature version rule after runtime qualification; current package version is 0.263.0-beta. Documentation adoption alone does not change that version.
- Preservation: Keep every existing working capability and all unrelated work. Edits are confined to this feature's existing owners and their necessary declarations/tests/documentation. No destructive or external action.
- Out of scope: Pinch, rotation, kinetic/inertial scrolling, edge handoff, two-finger pan, general touch admission of other controls, Mouse/Pen gesture redesign, UI replacement and consumer publication.

### Architectural boundaries

- Governing clauses and relevant owners have been inspected; this contract is subordinate to HAL-002/002A.
- Widget policy is static. Button keeps ordinary touch activation. ScrollArea declares gesture-only touch participation. Display-only widgets gain no input.
- Engine authority selects and routes the qualified pan; widgets own only their local press or scroll state.
- Applicable FORBID rules include capture stealing, hidden globals, cross-widget references, synthetic input, destructive cancellation of an unrelated contact and broad paint invalidation.
- Reinvention check: use existing registry, capture, domain authority and scroll state; no alternate recognizer transport.
- Existing examples remain supported. An optional demonstration cannot replace the canonical acceptance tests or introduce application-local gesture behavior.

## 1. Relationship to existing contracts

HAL-112's scoped contact activation still permits Button press capture. This SPEC adds one engine-owned cancellation of that unfinished Touch press when the first eligible ScrollArea gesture wins.

HAL-128's existing single-winner and capture invariants remain binding. An already winning gesture is the recorded GestureRegistry::committed_candidate with is_committed true. A tentative arbiter choice is not that state. This SPEC does not permit replacing a recorded winner.

For simultaneous pending requests involving this new touch-content Scroll candidate, section 3 explicitly makes content scrolling the fallback after an eligible non-Scroll request. This is an explicit profile rule, including the necessary correction to the current Touch-primary-Scroll implementation order. It must not be described as preserving that current implementation order. Mouse/Pen and unrelated gesture profiles retain their existing order.

HAL-131A remains locked. This SPEC supplements it with explicit recognition and terminal policy while preserving exclusive scroll ownership, user-intent dominance and no edge handoff. Historical candidate text is retained for review provenance only.

## 2. Qualified recognition facts and static admission

The platform converts physical coordinates to window-logical coordinates once. InputFusion owns native contact identity and the immutable physical press. Movement facts must be attached to the existing qualified input envelope at ordered ingress; consumers must not reconstruct an earlier Move from a mutable table that has already observed later Move/Up events.

For each contact, preserve its original Down position, current sample position, preceding sample position/delta, native-contact lineage, window and physical-press identity. At Down, latch whether it is the first currently live native touch contact in that window. This pan-eligibility fact never changes during that contact's lifetime.

Only that first contact may begin a content pan. A subsequent contact cannot become eligible merely because the first one ends. After all contacts end, a fresh first Down may qualify. A later contact neither cancels nor takes over an active pan; its existing ordinary Button routing remains available. Domain exclusivity is checked separately at each new recognition attempt.

The existing static touch-participation contract must distinguish ordinary pointer delivery from gesture-only participation. Existing opted-in controls retain ordinary delivery. ScrollArea declares a Scroll candidate and gesture-only Touch delivery: ordinary Touch Down/Move/Up must not enter its Mouse scrollbar, proximity or generic pointer-drag handlers. Mouse handling remains unchanged. Touch beginning on a scrollbar thumb or track follows this same content-pan profile.

Register candidates from the admitted original Down path through the existing registry. An ancestor ScrollArea remains observable for recognition even while a descendant Button owns press capture. Recognition must not depend on bubbling that captured Move through ordinary widget handlers. No candidate may be introduced from a later hover/hit path.

Use an additive, typed, engine-delivered touch-motion/intent hook on Widget and explicit qualified lifecycle facts in EventContext. Do not change the public meaning of existing FusedInputEvent variants or fabricate Mouse events. The existing gesture-request and gesture lifecycle mechanisms remain the sole arbitration path. The typed hook and lifecycle context must carry enough press/pointer/session identity to distinguish two contacts on the same widget.

## 3. Threshold, axis, eligibility and pending selection

Before a winner exists, a newly delivered TouchMove may request content scrolling when all of these hold:

1. The contact has its immutable first-contact eligibility.
2. Its original qualified press/session is still semantically live and admitted.
3. Euclidean distance from Down is at least 4.0 window-logical pixels, reusing the current InputFusion DRAG_DISTANCE_THRESHOLD metric and inclusive comparator.
4. A live originally registered ScrollArea supports the selected axis and has strictly positive current scroll range on that axis.
5. Existing capture, interaction-domain authority, overlay and widget-availability checks permit admission.

Choose the axis from the accumulated Down-to-current displacement: vertical when abs(dy) >= abs(dx), horizontal otherwise. Axis is locked when a ScrollArea wins and never changes during that pan. A refused tentative attempt does not lock an axis; only a newly delivered qualifying Move may create another request. Up alone cannot recognize or start a pan.

Resolve an eligible non-Scroll request before the new content-Scroll fallback, preserving the existing ordering among those non-Scroll requests. Among eligible content-Scroll candidates for the selected axis, choose the deepest candidate on the original Down path. A zero-range or unsupported-axis candidate is not eligible; an ancestor may be selected initially instead.

Positive range is sufficient even if that owner is already at its boundary in the attempted direction. Once selected, reaching or starting at the boundary does not cause ancestor fallback or handoff. Do not switch axis to find an otherwise eligible owner.

Existing winning gestures, non-Button capture, an unavailable owner and an occupied incompatible domain refuse admission. The pending batch already taken by the resolver may be consumed. Refusal does not create automatic retry, replay or fallback within that batch.

## 4. Existing scroll authority and competing input

There must be one canonical scroll-authority value in the existing interaction domain. Refine that existing authority with explicit source and qualified-contact provenance where needed; do not add a competing active-pan authority or infer authority from widget visual state.

A first eligible Touch Down explicitly ends an existing wheel-only routing authority in its own domain, provided it is not backed by another active pointer interaction. This is the touch counterpart of the existing pointer-down wheel-authority termination. It does not cancel pointer capture, another active pan or a different domain. Later Touch Down cannot clear an active pan.

At successful pan acquisition, the existing scroll authority names the same ScrollArea and qualified press as the gesture winner and pointer capture. Mouse Down/Up, unrelated Touch terminals and wheel hit-test results cannot clear or overwrite that active pan authority. Only the matching accepted terminal or an existing admitted lifecycle termination can end it.

While this pan owns a domain, other requests to mutate scroll offset in that domain are not accepted, including wheel aimed at the same owner, wheel aimed elsewhere, keyboard scrolling and programmatic reveal/scroll intent. They are not queued or replayed after release. Existing geometry/layout updates and bounds clamping remain valid. Ordinary keyboard activation, focus and pointer/Button routing remain unchanged; independent domains remain independent.

Enforce this through the existing scroll-intent admission owners, not by a new global switch, by dropping all keyboard input, or by changing focus. Preserve all current behavior outside the active pan interval.

## 5. Transfer of an unfinished Button press

An unfinished Button press is its still-owned interaction for this qualified physical press, whether activation-eligible or permanently first-exit-disarmed. A visual pressed flag or Hard capture alone does not establish this role.

The engine must obtain an explicit owner-local unfinished-press query from the existing Widget policy, defaulting to false and implemented by Button using its actual PointerId ownership. This query supplements, and never replaces, proof of the same live mount, window, domain, PointerId, immutable physical press and gesture session. No downcast or direct Button reference from ScrollArea is permitted.

Immediately before transfer, validate all of the following:

- The current capture belongs to that live Button's unfinished Touch press.
- The selected ScrollArea is live, registered for the same session and admitted by sections 2-4.
- No recorded gesture winner already exists.
- No routed terminal has retired the press; both owners pass lifecycle and input-availability checks.
- Every fallible capture/admission condition succeeds before changing capture, Button state, winner or candidates.

A future raw release already ingested is not delivery of a semantic terminal and cannot invalidate an earlier ordered movement.

Perform one synchronous transition through the existing registry and capture manager. No application callback, routed event or deferred tree mutation may interleave within its validation/publication boundary. Before callbacks observe success, the selected ScrollArea is the sole winner and sole capture owner for the unchanged press/session. There must be no winner without its capture.

After revoking the former capture, notify the Button exactly once through its capture-owner cancellation hook. Clear its owned press even if it was already disarmed; never emit activation. This notification is required even when Button is not a gesture candidate. Preserve its independent keyboard pulse and focus, and invalidate changed Button paint narrowly.

Settle other candidates using the existing loser lifecycle. The Button capture notification is local owner cancellation, not native TouchCancel, synthetic Up, blur or full pointer-session teardown. Cleanup qualified to the former owner cannot release the new owner's capture.

Deliver the ScrollArea start callback once, after the former press is cleared, while the selected mount is still the winning capture owner. Duplicate/stale requests cannot repeat it. If legitimate lifecycle termination makes that owner unavailable, use qualified teardown; do not call a stale mount or select a replacement winner.

For refusal while the source press remains valid, preserve its exact eligible/disarmed state and capture, retain tentative candidates/session, install no winner and emit no transfer cancellation, loser notification, start or scroll movement. Never cancel first and try to reconstruct the Button. Actual routed cancellation, disable, removal or overlay takeover retains terminal authority; retired state is never resurrected.

This exception admits no other control, pointer type or capture purpose. With no current capture or the same owner, preserve ordinary successful acquisition. For every Absolute gesture path, capture refusal must precede winner publication and loser/start callbacks; repair the current resolver bug that logs AlreadyCaptured after recording a winner. Preserve the existing RelativeLocked denial behavior.

## 6. Motion and visible state

At acquisition, snapshot pregesture scroll intent needed by the existing cancellation contract. This snapshot is not a copy to restore over future viewport/content geometry.

Use the current effective visible offset as the direct-manipulation baseline, including applicable layout-anchor tail slack. Do not start from a smoothing target ahead of the displayed content. Retire the superseded anchor/reveal/retained-restore intent through the existing ScrollArea owners. Preserve the orthogonal axis.

The first winning sample applies the complete accumulated Down-to-current displacement exactly once. Later samples apply only their incremental movement since the preceding delivered sample. Content follows the finger, so subtract finger delta from scroll offset on the locked axis. Clamp against the current valid range.

At a bound, consume the input sample even if the offset cannot move. Reversing direction then moves immediately; there must be no dead zone caused by accumulated overshoot. Repeated position is a no-op.

Update the existing ScrollState, ScrollContainer presentation and scroll links coherently. The selected visual axis tracks the new logical offset immediately using the existing jump/synchronization mechanism. This profile adds no inertial velocity, smoothing lag or animation after release. Preserve unrelated presentation and required scroll-dependent layout/paint invalidation.

The recognition envelope that acquired the pan must not also apply a second ordinary PointerMove or typed update. After acquisition, typed pan updates go only to the matching winning capture owner. Local state and every lifecycle callback must qualify the same pointer/press/session; cancellation of another contact on the same ScrollArea cannot cancel this pan.

## 7. End, cancellation and lifecycle

Matching routed Touch Up consumes its final position delta once for an already active pan, retains the final clamped scroll intent and ends its capture, gesture and domain authority through the existing lifecycle. An Up without a prior winning Move does not begin a pan. No synthetic release is sent to the old Button.

Native Cancel and other accepted cancellation restore the snapshotted pregesture scroll intent through the existing scroll resolver, clamped to the owner's current valid geometry. Preserve current content extent and viewport extent; never restore old geometry. No restoration of the former Button press or activation is permitted.

If the owner is removed or unavailable, existing lifecycle teardown clears qualified authority without invoking a destroyed mount. Duplicate or stale terminals are idempotent and cannot end a newer contact lifetime, another pointer, Mouse capture or independent keyboard activation.

Range changes while panning use current bounds. Explicit geometry correction is not a competing programmatic scroll request. Normal release and cancellation are different outcomes and require separate routed tests.

## 8. WASM canvas ownership

The same InputFusion, arbitration and ScrollArea behavior applies on native and WASM. Keep the current single platform coordinate conversion and native Touch transport.

Halcyon must establish CSS touch-action: none on its own rendering canvas before the browser can begin a touch sequence on it. Keep this scoped to the application canvas, not document/body controls. Use Halcyon's existing WASM window/canvas initialization owner and web-sys/winit access, without a second JavaScript input listener or a consumer-specific gesture implementation. Respect an already established equivalent canvas policy.

PointerDown preventDefault by itself is not the acceptance proof for browser touch reservation. Preserve the existing native PointerEvent/Touch path, terminal cancellation and focus behavior. No vendored input rewrite is required merely to provide this canvas policy.

A successful WASM compile proves compilation. Browser observation must separately establish native contact ingress and changing ScrollArea content offset; only an actual device observation establishes iPhone/Android interaction.

## 9. Acceptance traces and bounded verification

Use real qualified RawInputEvent::Touch -> InputFusion -> EventRouter routing in the existing mounted fixture. Do not seed the winner or call widget handlers directly to represent normal-path success.

| Test | Required result |
| --- | --- |
| touch_scroll_threshold_and_incremental_delta | Below/exact 4.0 Euclidean logical-pixel boundary; initial full displacement once; repeated position no-op; incremental motion; clamp then immediate reversal. |
| touch_scroll_button_handoff_armed_and_disarmed | Both Button states transfer; press/session identity is unchanged; capture and winner agree at start; exactly one old-owner cancellation; no click; narrow paint invalidation. |
| touch_scroll_capture_refusal_preserves_source | Actual valid-source refusal boundary; exact source state/capture and tentative candidates survive; no false winner/start/cancellation/offset/replay. Include protected pre-existing winner and non-Button capture. |
| touch_scroll_nested_axis_and_exclusive_bounds | Deepest original-path eligible owner; horizontal/vertical support; vertical dominance tie; fixed axis; positive-range owner initially at a bound; no edge handoff. |
| touch_scroll_contact_lifecycle_isolation | P pans; Q on the same node cannot pan or cancel P; Q never becomes eligible later; fresh first contact can pan; ordinary Q Button and keyboard/focus behavior survives. |
| touch_scroll_batched_terminal_order | Down, qualifying Move and future Up ingested before routing; earlier Move remains valid; active Up final delta applies once; Up alone cannot start; duplicate/stale terminals and contact-ID reuse are isolated. |
| touch_scroll_cancel_restores_current_geometry_clamp | Cancel restores intent under changed current bounds while preserving current metrics; normal Up retains result; next interaction starts correctly. |
| touch_scroll_preserves_current_user_authority | Same/other-target wheel, keyboard scroll and programmatic reveal cannot mutate active domain or replay later; independent domain works; wheel-only authority terminates only under the admitted rule; visual and target baseline differ. |
| touch_scroll_scrollbar_hit_admission | Touch on thumb/track enters content-pan only; no legacy mouse drag/proximity state; existing Mouse scrollbar behavior survives. |
| touch_scroll_platform_logical_coordinates | Real platform conversion at DPR 2 and 3 yields the same logical threshold and preserves window/contact identity. |
| touch_scroll_motion_facts_survive_batched_ingress | Earlier qualified envelopes retain their own positions/deltas and initial eligibility after later ingress and terminals. |

Retain the existing touch_contact_ and positioned_consumer_absolute gates. Extend the existing Absolute-refusal fixture to prove rejection occurs before any winner, loser cancellation or start callback. Cover eligible pending non-Scroll priority and exact same-owner/uncaptured success without broad unrelated test expansion.

Run the explicit WASM command in §0. If a required target or locked offline dependency is unavailable, report that gate failure; do not silently change target, online policy or qualification criteria.

Report four evidence classes separately: core routed behavior; WASM compilation; browser runtime; physical device. Mounted fixtures establish only the first class.

## 10. Sources

- SPEC-HAL-002-Constitution-and-Architectural-Invariants.md and SPEC-HAL-002A-Constitution-Amendment-Guidelines.md.
- SPEC-HAL-112-Button-Interaction-and-Activation-Contract.md.
- SPEC-HAL-128-Pointer-Capture-and-Gesture-Arbitration.md.
- SPEC-HAL-131A-Scroll-Substrate-and-Input-Arbitration.md.
- SPEC-PM-018-Semantic-Mutation-Without-Retained-Paint-Invalidation.md.
- The 2026-11-20 reviewed proposal “Touch Scroll: Transfer of an Unfinished Button Press” was superseded by this SPEC. Section 5 contains the binding transfer contract.
- Existing owners: src/core/input_fusion.rs, src/core/event/mod.rs, src/core/event_router.rs, src/core/widget/mod.rs, src/widgets/button.rs, src/widgets/scroll_area.rs, src/core/scroll_state.rs, src/core/scroll_container.rs and src/platform/window.rs.
