# SPEC-HAL-399: Kinetic Touch Scrolling
Status: Adopted / Active / Binding. Date: 2026-11-21.
Authority: JC directly approved native-feeling flick/inertia as part of prioritized cross-device Halcyon functionality. This SPEC deliberately extends HAL-396 section 6's no-inertia Touch profile only AFTER a qualified non-canceled Touch Up. HAL-131A remains locked and unchanged.
Owner: the existing ScrollArea interaction/motion and ScrollContainer presentation owners, not ScrollState, an OS scroll view, or a second registry.

## Scope and verification

- Objective: A touch pan follows the finger precisely, then a genuine flick continues decelerating after release; next touch stops it immediately; no change to mouse, trackpad, wheel, keyboard or geometry contracts.
- Consumer scenario: Mobile content follows direct touch manipulation and decelerates after a flick, while all controls remain touch-responsive and desktop behavior is preserved.
- Authority: HAL-002/002A, HAL-086, HAL-112/128, locked HAL-131A, HAL-396, HAL-397, PM-002/005/018. This new SPEC supersedes only HAL-396 section 6's explicit exclusion of post-Up inertial motion and supplements its exclusive-owner terminal policy.
- Source baseline at adoption: ScrollState is pure geometric offset with explicit no inertia/timer restriction; ScrollArea HAL-396 touch currently jumps visual offset to finger displacement, then ends motion on Up; ScrollContainer has existing presentation smoothing for wheel but no Touch release velocity. QualifiedTouchMotion and TouchScrollMotion currently carry no timestamp, while InputFusion events already carry monotonic event times.
- Owned surface: timestamp-qualified touch sample ingress, same ScrollArea active motion state, existing scroll-intent admission, frame update scheduling, scroll links, retained invalidation and routed tests.
- Forbidden: smoothing lag while finger is down, extra authoritative offset, browser scroll/CSS momentum, layout/paint mutations, frame-rate-dependent math, edge chaining, synthetic Wheel events, foreign-domain interrupts, hidden feature switch, changing HAL-131A source.
- Verification: deterministic motion samples and frame times, bounds/range changes, capture/keyboard/wheel/cancellation, native regression and actual iPhone flick.

## Architectural boundaries

- Governing: HAL-002, 086, 128, 131A, 396, 397 and this exact post-release continuation.
- Widget: existing ScrollArea is still the only content scrolling widget/owner; no duplicated ScrollState.
- Interaction: kinetic continuation after successful release of the same qualified native Touch pan.
- FORBID: no OS semantic scroll, new global momentum owner, hidden time source, ScrollState velocity/timers, mouse/trackpad behavior changes.
- Existing: ScrollArea jump_touch_scroll/apply_touch_scroll_motion, ScrollContainer presentation, GestureRegistry, domain ScrollIntentAdmission and existing frame ticker.
- Reinvention: reuse these owners and the normal frame update; do not build a new animation engine.
- Demo: examples/ uses ordinary ScrollArea and real routed gesture; Escape remains framework-owned.

## 1. Motion semantics
1. HAL-396 recognition, threshold, first-contact policy, original axis, capture transfer, Down/Move/Up/Cancel and direct-manipulation displacement are unchanged. As long as finger is held, displayed offset tracks each qualified delta immediately with no smoothing lag. The first winning sample still applies accumulated Down-to-current displacement exactly once.
2. Only a matching successful Touch Up of an already committed content-Scroll gesture may start inertial continuation. Up applies the final finger delta once before determining release velocity. A Tap or rejected/never-winning pan NEVER starts inertia.
3. Add a monotonic timestamp to the existing qualified touch-motion envelope at InputFusion ingress (not a mutable contact lookup after a later Up); preserve original native contact/PointerId/press/window/domain identity. Reject negative/nonfinite/out-of-order timestamps without motion. Do not invent timings from frame timestamps or infer velocity from CSS pixels after resize.
4. Keep up to the last 80 milliseconds of unique delivered movement samples for the winning physical contact. Calculate recent finger-axis velocity from displacement divided by positive elapsed sample time; require at least two distinct samples separated by at least 8 ms. Use the actual last delivered Up position if it moved. When the latest meaningful movement precedes Up by more than 100 ms, release velocity is zero. Positive scroll-axis velocity is opposite finger velocity; do not borrow velocity from Mouse/Wheel.
5. Clamp absolute release velocity to 4000 logical px/sec. If its magnitude is below 80 logical px/sec, settle directly at final clamped offset with no kinetic phase. Ignore nonfinite coordinates/times/velocities rather than propagating them into scroll state. On an admitted release above threshold, create a single motion continuation owned by that same ScrollArea/domain.

## 2. Deterministic deceleration
1. Let s0 be final visible scroll position, v0 the qualified signed release content velocity and t nonnegative elapsed wall-clock seconds since accepted Up. Use lambda = 4.0 per second. The unbounded displacement is v0 / lambda * (1 - exp(-lambda * t)); projected position is clamp(s0 + displacement, current range).
2. Evaluate position from the fixed release origin and elapsed monotonic time, NOT by adding per-frame velocity or a frame-dependent series of deltas. This yields the same position at a given elapsed time for different frame cadences. Preserve the orthogonal axis.
3. Stop when projected |v0 * exp(-lambda*t)| is below 8 logical px/sec, when a reached bound prevents further travel in the current direction, or when explicit cancellation applies. Snap only the insignificant final tail to the derived clamped rest position; never overshoot or bounce. Dynamic viewport/content changes re-clamp current and rest positions to present geometry without restoring obsolete geometry.
4. Each actual change uses the same ScrollArea scroll-state setter, scroll-link publication, visual sync and retained scroll-dependent invalidation already used by HAL-396. Motion state is interaction/presentation state in the existing ScrollArea owner, NEVER in geometric ScrollState or a parallel global store.
5. The existing update/ticker owns frame scheduling for the period of active inertia; active motion requests frames and stops requesting them when settled. No background timer, delayed event replay or platform requestAnimationFrame loop outside the existing application loop.

## 3. Authority and interruption
1. The prior touch capture and gesture terminate normally at accepted Up; inertial continuation holds only one qualified scrolling-motion lease in the SAME existing interaction domain/scroll-intent authority, not an artificial pressed pointer or gesture winner.
2. A new genuine Touch Down that is admitted for that window/domain immediately interrupts its existing kinetic motion, before any fresh touch recognition. It does not alter other domains. The new touch is independently evaluated under HAL-396 first-contact rules. Physical hardware Mouse/keyboard input remains in its original priority and can interrupt kinetic movement through its established scroll-intent path.
3. An admitted Wheel/trackpad/keyboard/programmatic scroll or qualified focus reveal ends kinetic continuation BEFORE applying that new request; no mixed target values, queued deltas or restoration after interruption. For unrelated mouse movement/hover do not cancel kinetic motion.
4. Gesture Cancel while the finger is still down retains HAL-396 original-intent restoration and NEVER begins inertia. A lifecycle cancellation after release terminates only the current matching motion lease, never rewinds scroll position to an obsolete pre-pan snapshot or reactivates a Button.
5. Removing/disabling the owner, changing domain/overlay authority or replacing the window terminates that motion before the next frame; stale callbacks never update a new mount. There is no parent handoff or boundary chaining at limits, preserving locked HAL-131A.
6. No kinetic motion may steal another widget's active drag or another contact's capture. If a currently active incompatible domain owner denies the new motion lease, preserve the released visible offset and settle without inertia. All admission is explicit, fail closed and mutually exclusive with other scroll intent owners.

## 4. Acceptance
1. Real routed Down/Move/Up touch tests: direct 1:1 displacement while touching; slow release stops; fast release advances beyond finger endpoint; velocity sign, threshold, reverse, different axis and per-contact isolation.
2. Deterministic time tests at 30/60/120 Hz and uneven frame intervals produce equal position at the same elapsed time within numeric tolerance; no inertial frame continues after terminal. Bounds, changing extents and immediate reversal behave without bounce or ancestor transfer.
3. New touch interrupts before applying its first sample. Wheel, trackpad, keyboard scroll, focus reveal and programmatic scroll each have the original expected effect after stopping inertia. Mouse hover, drag, capture, TextField editing and popup interaction remain unaffected.
4. Physical mobile test using a Halcyon form with TextField, ComboBox, Checkbox and ScrollArea at 320/390 CSS widths and DPR 2/3: drag directly follows the finger, a fast flick continues after release, the next tap stops it, and subsequent taps on TextField/ComboBox/Checkbox work. Desktop mouse/keyboard regression and a locked wasm32 release build are required at the changed boundary. Native numerical tests do not establish physical-device success.
