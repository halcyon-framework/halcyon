# SPEC-HAL-365: Focused Keyboard Response and Presentation Timing

**Author:** JC\
**Status:** Proposed — requires explicit JC adoption\
**Date:** 2026-10-07\
**Owner:** Existing platform-input, keyboard-authority, binding, and canonical App frame owners\
**Implementation status:** Not Implemented\
**Verified baseline:** `552f8d61d68bcc6f39179062789e47b3317c75e7`, `0.204.1-beta`

MUST and MUST NOT below describe the proposed contract upon adoption. This
document does not activate runtime work, amend the Constitution, adopt a draft
dependency, or claim native proof. Existing binding authority remains effective
until JC accepts the scoped amendments in §8.

## Publication Pre-Flight

- Objective: decide one generic framework contract for authorized keyboard
  response delivery and a truthful presentation-time proxy; no product semantics.
- Reading: live AGENTS, PRE-FLIGHT, ENGINEERING, PROJECT-ENTRY, 002/002A;
  governing 020B/C, 176, 328, 329 and 361; relevant 020/020A, draft 018/033/041,
  adopted 309, advisory Design Intelligence, PM-000 and matching PM-014/018.
- Evidence: live source at the baseline above and read-only consumer research
  on keyboard-response and presentation-timing requirements. Consumer research
  supplies use cases, not framework adoption authority.
- Changes: this proposed SPEC only. No runtime, test, Cargo, release or consumer
  edits; documentation source/reference/status review only; no build or test run.
- Reuse: existing Widget capture/target event route, InputFusion, Shortcut,
  InputPattern, BindingRegistry, AppFrameAttemptId and frame-realization receipts.
- Halcyon policy: generic captured input, not a new widget/category or third
  keyboard mode. Escape ownership and focus/capture exclusivity remain binding.
- Forbidden: parallel input authority/matcher/registry, async semantic mutation,
  renderer callbacks into consumers, product cue/attempt/evaluation payloads.
- Release/Git: proposed documentation has no version bump or accepted-architecture
  changelog entry. A reviewable documentation commit is not adoption; no push.

## 1. Existing Boundary and Concrete Gaps

No existing public path satisfies all four requirements together.

| Baseline evidence | What it actually establishes / lacks |
| --- | --- |
| `src/core/widget/mod.rs`, `Widget::handle_event`, `EventContext::request_keyboard_capture`; 328 §§2.2–3.9 | Existing public focused-and-captured Widget boundary; focus alone grants no input authority. |
| `src/platform/window.rs:220–228,316–439` | Converter samples an Instant, mixes physical named keys with a reduced logical character projection, and suppresses clipboard Key Down/Up before fusion. |
| `src/core/app.rs:11249–11275,11862–11884,12067–12128`; `AppRoot::handle_fused_input_modal_first` | Shift+F, F9, F12 and app/screen shortcuts can precede captured-widget routing. This ordering conflicts with 328 §2.3 and 020B's widget-before-app rule; it is not a measurement guarantee. |
| `src/core/event/mod.rs`, `KeyEvent`, `RawInputEvent`, `FusedInputEvent`; `InputFusion::process_key` | Raw/fused keys retain time/repeat/modifiers, including unbound keys. Public Widget KeyEvent has neither physical identity nor ingress time. |
| `src/runtime/actions.rs`, `Shortcut`; `src/runtime/binding_registry.rs`, `InputPattern` | Existing exact key-plus-four-modifier values/lookup, without a distinct physical-key basis. |
| `Widget::on_capture_leave`; `EventRouter::notify_capture_leave` | Existing lifecycle notification lacks a typed interruption cause/time and cannot notify an already-destroyed widget. |
| `Screen::handle_input` | Earlier fused input includes surviving timestamps, but is not capture authorization and cannot recover adapter-suppressed input. Draft 018 does not replace binding 328. |
| 329 §§3–5, materialized as `SPEC-HAL-328A-Category-K-Amendment.md` | KeybindEditor edits one Shortcut, clears on bare Backspace, excludes Tab/Escape and releases capture after a stroke. It is not this general response boundary. |
| 361 §§1–3; `src/core/frame_realization.rs`; `WgpuRenderer::submit_and_present` | Exact attempt/surface and actual present-call facts, but no present timestamp. Latest-only overwrite can lose an intermediate receipt. |

Draft 033's physical-key and timing descriptions are design evidence, not proof
of the current payload. Draft 041's frame-time substitution is not authority to
replace an input ingress Instant. The 020 §C / 020A §C.1 ActionId representation
contradiction remains outside this contract: no new allocation or persistence
scheme is needed for response facts, existing Shortcut equality or binding lookup.

## 2. Public Consumer and Delivery Authority

**K1 — Existing capture owner.** The public consumer boundary is
`Widget::handle_event(Event::Key, EventPhase::Target)` for the live, enabled,
focused Widget that holds framework keyboard Capture in the active interaction
domain and admitted input root. Entry remains explicit activation through the
existing action/request machinery. Neither a Screen callback, an observer handle,
focus alone nor an application flag grants capture. No new framework widget,
keyboard mode or complete future scope stack is required.

**K2 — Order.** For a normalized keyboard source event, canonical App routing MUST:

1. Reconcile the active window, existing modal/input-root boundary and capture
   owner. An excluded/suspended owner receives the §3 interruption fact rather
   than input through a modal or inactive-window boundary.
2. Resolve an engine Hard reservation. For this contract the keyboard reservation
   is Escape, with any modifiers; perform the existing unconditional release.
3. Offer every other Key Down/Up to the captured owner, including wrong and
   unbound keys, before Screen, app/global, fullscreen or diagnostic shortcuts.
4. If handled, stop this source event's command propagation and its derived
   Shortcut/paired Text residue. A source association, not timestamp uniqueness,
   MUST identify that residue; two unrelated events may share an Instant.
5. If declined, preserve 328's Tab traversal and modifier-command fallthrough.
   Other plain/Shift keys are dropped without navigation or command dispatch
   during Capture. An actual
   lower-scope command consumption is observable as `ConsumedElsewhere` (§3).

Input normalization MUST emit the source facts before policy filtering. The
platform adapter MUST NOT query widget state or become a second authority owner.
Existing text/clipboard projection belongs after the captured-owner decision;
text editors retain exactly one clipboard/edit consumer, with no duplicate
Key/Text action. Outside this captured boundary existing navigation and binding
semantics are unchanged; this is not implementation of the full 020B scope stack.

### 2.1 Explicit Reservation Decisions

| Input | Proposed disposition while a valid consumer holds Capture |
| --- | --- |
| macOS Cmd+A/C/V/X/Z/Y; Windows Ctrl+A/C/V/X/Z/Y, including additional modifiers | Observable ordinary keys, not an adapter suppression/reservation. Handling prevents clipboard/edit/app side effects; declining may delegate through existing editing/command ownership. Literal Ctrl remains Ctrl on macOS. |
| Shift+F | Observable ordinary chord. If declined during Capture, drop under 328 §2.3; do not toggle fullscreen. The existing fullscreen command remains outside this captured boundary. |
| F9 / F12, with or without modifiers | Observable ordinary keys first. Plain/Shift decline is dropped; Ctrl/Alt/Meta decline may take the ordinary eligible app-command fallback. Blueprint export/debug outline behavior is not an engine-safety Hard reservation. Existing command eligibility outside capture is not expanded. |
| Escape, with any modifiers | Engine-reserved release, never a completed keyboard response. Publish `EscapeReserved` with the source facts; no veto, rollback or widget-specific Escape action. |
| Tab / Shift+Tab, without Ctrl/Alt/Meta | Widget-first under 328 §3.7. A general consumer may handle and measure it. If declined, publish `TabTraversal`, release and navigate/auto-capture normally. Category K still declines Tab under 329. |
| Other modified Tab chords | Ordinary captured modifier chords; app fallback only after decline. They do not acquire a new hard traversal reservation. |
| OS/application switching or a key intercepted before Halcyon ingress | No invented key, response timestamp or latency. An observed window deactivation interrupts (§3). If the OS supplies neither key nor lifecycle evidence, exact response detail is unavailable; absence/consumer timeout is not a completed response. No OS hook/global key interception is authorized. |

Expected-binding exclusion is never a substitute for delivery or a typed
interruption. A future engine-safety reservation requires explicit authority and
the same observable source/cause contract; apps cannot silently promote ordinary
shortcuts to Hard reservations.

## 3. Interruption and Ownership Lifetime

**K3 — Optional terminal observation.** Add one opt-in, read-only
`KeyboardCaptureObservation` through the existing EventContext/capture lifecycle.
Enrollment requires the current context owner to satisfy K1; otherwise it returns
a typed not-authorized result. Enrollment observes authority and MUST NOT request,
grant, restore or mirror capture. Clones share one observation interval, qualified
by framework identity, WindowId and TreeWidgetId. There is at most one active
observation interval per domain's captured owner; no per-key history or event bus.

An observation has immutable current-authority evidence and one sticky terminal
interruption fact. The engine retires it synchronously at the relevant routing or
lifecycle boundary, before that interval can accept more input. Surviving clones
retain the terminal fact even if the widget is destroyed; no callback targets a
dead widget. Existing `on_capture_leave` remains the ordinary mounted-widget
notification, without veto or altered editing semantics. Consumers read the
observation in their normal input/application lifecycle; no polling frame loop,
background UI mutation or observer-driven render demand is added.

The typed causes are `EscapeReserved`, `TabTraversal`, `FocusLost`,
`CaptureReleased`, `WindowInactive`, `ScopeSuspended`, `OwnerDestroyed`,
`ApplicationStopping`, and `ConsumedElsewhere`. A key-caused fact carries its
original input origin, ingress Instant, identities, modifiers and repeat flag.
A normalized focus-loss fact carries that event's ingress Instant. A purely
framework transition carries an engine transition Instant explicitly labelled as
such, never as key ingress. `ConsumedElsewhere` identifies the actual command or
owner disposition and source event; no command is invented for a dropped key.

Window deactivation or overlay suspension ends the observation interval even
where 328 preserves authority for restoration. This does not change that
restoration policy or re-fire capture entry. Restoration cannot reactivate an old
terminal observation or resume an old response measurement; fresh explicit
consumer enrollment is required. Observation retirement never rolls back an
already-completed earlier response.

Focus/capture loss, suspension, removal and reserved/delegated responses MUST NOT
be reported as completed-attempt latency. Halcyon supplies causes/facts; the
application owns abandonment, retry, timeout and evaluation.

## 4. Key Meaning and Existing Binding Values

**K4 — Separate facts.** Extend the existing RawInputEvent → FusedInputEvent →
KeyEvent lineage with one framework-owned physical-key identity, the original
logical-key meaning, original ingress Instant, originating window and source
association/order. Preserve Down/Up, repeat and modifier snapshot. Keep committed
UTF-8 text separate from physical keys and logical key meaning. A character's
first code point, ASCII lowercase, layout label or composed text MUST NOT stand
in for physical identity. Navigation translations do not rewrite these original
facts, and Capture does not apply hjkl/i navigation translation.

The physical type belongs to Halcyon's existing input module, never to a product
or winit outside the adapter. Known codes identify keyboard positions independent
of active layout, including letters, digits, punctuation, named editing/navigation
keys, function keys, numpad keys and left/right physical modifier keys. Numpad
Enter/digits are distinct from main Enter/digits. An unavailable/unrecognized
physical code is explicit and its key event is still delivered; no guessed
Character substitute or silent drop. Logical characters retain their complete
logical value; committed text/IME is not a second response event.

**K5 — One binding mechanism.** Add a tagged physical-key alternative to the
existing framework KeyCode used by Shortcut/InputPattern, and framework helpers
that project a known physical key plus Modifiers into those existing values.
Existing logical Shortcut constructors/equality retain their meaning. The
consumer chooses the physical or existing logical basis explicitly; a single
lookup uses one basis, not two action dispatches. Existing BindingRegistry
tables, priority, exact equality and ActionId dispatch remain owners. This unit
does not add mixed-basis global dispatch, a second matcher/registry, or ActionId
allocation rules. Applications may use canonical Shortcut equality or the
existing registry to evaluate bindings; they must not reconstruct physical keys
from text or build a parallel key-pattern language.

**K6 — Modifier limit.** Chord modifier identity is the existing aggregate
Ctrl/Alt/Shift/Meta snapshot with exact required flags: extra or missing flags
distinguish patterns. Ctrl is not automatically Cmd; platform-command constructors
remain an explicit application choice. Left/right modifier *key events* retain
physical identity, but side-specific *chord requirements* are not supported by
this aggregate contract. Side-specific, AltGraph, Fn or lock-state source tokens
that require an unrepresented modifier distinction remain explicitly unsupported
at consumer import/compatibility admission; retain the original token and reason,
never silently collapse it. No independent AltGraph-vs-Ctrl+Alt claim is made.

Pure modifiers, Up events and repeats remain observable facts. The application
declares its qualifying-response rule independently of binding success. An
unbound/mismatched qualifying Down cannot disappear because registry lookup
returned None. Halcyon does not decide correctness or which cue is active.

## 5. Response Clock

**K7 — Ingress sample.** Sample `std::time::Instant` once at entry to canonical
platform normalization of the native KeyboardInput event, before shortcut,
clipboard or capture policy. Move any earlier framework shortcut handling behind
that sample; do not resample in a Widget callback. This is framework ingress,
not hardware actuation, OS event creation or callback completion time.

Copy that exact Instant through raw, fused and authorized target delivery,
including interruption facts for consumed/reserved keys. No frame-time rounding,
coalescing, timestamp clamping or consumer `Instant::now()` may replace it.
Maintain source order for equal samples. Synthetic input must carry explicit
synthetic provenance and cannot claim native measurement evidence. A response
queued before a cue proxy does not become post-cue merely because delivery is late.

The input and §6 presentation samples use the same in-process monotonic Instant
domain. Consumer latency, when its own attempt is valid, is the checked difference
between qualifying ingress and its associated presentation proxy. Negative order,
missing proxy, stale capture/window or interruption yields no completed latency.
Frame/animation time remains owned by its existing timing authority; this contract
introduces neither a clock service nor a second animation tick.

## 6. Cue-Associated Presentation Proxy

**P1 — Exact meaning.** Extend 361's per-surface receipt with an optional
`present_call_started_at: Instant`. The renderer owner samples immediately before
the actual `SurfaceTexture::present` invocation, after submission, and returns
that mechanical fact to the existing App recorder. Publish it only for an actual
issued present call, with the same exact AppFrameAttemptId/SurfaceId and existing
output/submitted/presented_called evidence. No present means None. Do not use
frame start, receipt publication, latest() read time or application callback time.

This proxy means **submission followed by present-call initiation for the exact
surface output**. It proves neither GPU completion, compositor latch, vsync,
scanout, physical cue visibility, occlusion nor human perception. Measured values
must be labelled present-call-proxy-to-framework-ingress intervals. Physical
display-onset latency remains unavailable under this contract.

**P2 — Bounded exact request.** Add an opt-in one-shot surface-presentation request
through AppContext in ordinary application lifecycle, reusing 361's recorder,
attempt identity, surface evidence and post-critical-section publication. A live
surface admits at most one outstanding request; an unknown surface, shutdown or
an already-outstanding request is typed rejection, without queued work. This is
an observation request, not a cue payload or rendering callback.

The request selects the next canonical render attempt begun after request
admission. Its result is pinned to that attempt and exact surface, never retargeted
automatically to a later successful attempt. The result is either its immutable
receipt/proxy or a typed not-presented/retired outcome. Unchanged scene, deferred
layout, missing target, Compile/output failure, surface removal and application
shutdown cannot fabricate a proxy. Dropping all request handles cancels its
pending observation; completion/retirement releases engine pending storage.

Resolved copies are caller-owned; the engine stores no history/receipt queue.
361's existing latest observer remains latest-only and may still overwrite
intermediate snapshots. The one-shot request preserves only its selected result,
so later receipt overwrite cannot substitute a later frame for the first cue.
Without either observation facility, no receipt/request storage or extra work is
introduced. Requests neither dirty layout/paint nor create continuous frames;
normal owner invalidation/frame demand must already admit the intended output.

**P3 — Application association.** The application publishes its candidate through
normal UI state/invalidation, then admits the request before the next render
attempt and retains that candidate unchanged on the target surface until the
request resolves. It owns the association of candidate/cue → request → exact
attempt/surface and the qualifying response. Halcyon certifies only mechanical
output facts, not the presence or meaning of a cue inside those pixels. An
application that changes its candidate before resolution must abandon that
association. No dummy custom-render participant, SceneNode identity or renderer
semantic callback is needed to obtain the receipt.

## 7. Minimal Implementation and Proof Boundary After Adoption

One future coherent framework unit may extend only the existing event/platform
conversion, fusion, canonical App/router capture path, input/binding value helpers,
capture lifecycle observation, frame-realization recorder and actual renderer
present-call fact. Reuse current UI phases and deferred mutation rules. Do not
implement the full scope stack, new widget, asset system or product behavior.
Public guide updates and release metadata accompany that additive runtime API,
not this proposed document. Product source-token interpretation and semantic
cue/attempt evaluation remain entirely consumer-owned.

Required owner-local mechanical proof:

- Known physical identity versus full logical/text facts, exact modifiers/repeat,
  original Instant and source order survive canonical ingress/fusion/target
  routing. Correct, wrong and unbound Downs all reach the same authorized owner.
- One consumed source cannot also execute a derived Shortcut/Text/app command;
  two events with equal Instants are not incorrectly paired. Text/clipboard's
  existing intended edit path still executes exactly once.
- Capture exclusivity, every §2.1 disposition and all §3 terminal causes, including
  held observation after owner destruction, without authority restoration or
  completed latency fabricated by a notification.
- Existing Shortcut/InputPattern/BindingRegistry exact matching for the chosen
  basis; logical compatibility, distinct numpad identity and explicit rejection
  of unrepresented source modifiers, without a new matcher or ID allocator.
- Real present-call timestamp association, no timestamp for non-presented output,
  exact one-shot retention despite latest overwrite, surface/app retirement and
  pay-for-use. No renderer-to-widget callback or receipt-driven frame loop.
- A deliberately delayed consumer callback retains the original ingress time;
  pre-proxy input remains pre-proxy. Missing/mismatched receipt never yields a
  completed interval. PM-018 invalidation law still governs changed UI state.

### 7.1 Separate Native Windows and macOS Acceptance

Both platforms require a real canonical App::run window and an ordinary
capture-eligible test consumer, entered by framework click/i/Enter. Headless or
synthetic event tests alone cannot close native proof. Record OS, keyboard layout,
native input source, exact implementation revision and each observed result.

1. On each platform, press a bound key, a different bound key and an unbound key;
   include a held repeat and modifier-only change. Prove target ownership, source
   timestamp preservation and one first qualifying response independently of
   matching. Change between two installed layouts and prove a known same physical
   position retains identity while logical meaning changes. Check main/numpad
   distinction on hardware that supplies it; otherwise report that proof pending.
2. Exercise literal Ctrl/Alt/Shift/Meta under the supported platform mapping.
   Explicitly try Cmd+C/V on macOS and Ctrl+C/V on Windows, Shift+F, F9 and F12 as
   wrong responses while handled: observe them without clipboard/fullscreen/
   diagnostic action. Check Tab handled and declined, Shift+Tab, and reserved
   Escape release/interruption. Do not infer Windows behavior from macOS results.
3. Exercise native application switching (Cmd+Tab / Alt+Tab), ordinary focus move,
   modal takeover and consumer removal. Observed lifecycle loss must end the
   observation without a key or response latency fabricated for OS-consumed input.
   Restoration does not resume the old measurement. OS-withheld keys without
   lifecycle evidence remain unobservable, not a passing input test.
4. Publish a visibly distinguishable candidate through ordinary UI mutation,
   obtain its exact one-shot surface/attempt receipt, then respond. Record the
   present-call proxy and ingress time separately from callback/read time. Delay
   ordinary observation across later frames and prove the selected result remains
   pinned. A failed/skipped attempt yields no onset proxy. A manual visual witness
   confirms the test candidate rendered, but does not calibrate physical onset.

Native proof certifies these transport, authority and proxy facts on the tested
platform. It does not certify hardware-to-photon accuracy or physical-display
reaction time. Failure or unavailable native evidence remains explicit, never
converted to a cross-platform green result.

## 8. Exact Scoped Authority Amendments Proposed for Adoption

These changes are specified here for review; existing owner documents are not
silently edited or promoted by publication of this proposal.

| Owner / exact clauses | Proposed amendment upon JC adoption |
| --- | --- |
| 328 §§2.2–2.3,3.2,3.7,3.6,3.9–3.10,18.2–18.3; 176 §§3.2,4.2,9.4 | K1–K3 make captured target precedence and observable interruption/lifetime explicit. Escape remains unconditional release; Tab widget-first and Category K limitations remain. Window/overlay authority restoration remains unchanged. Current pragmatic dispatch must enforce this without waiting for the full scope stack. |
| 020C §§C–D,J,L; existing input value owner | K4–K6 add an explicit physical basis in existing Shortcut/InputPattern and preserve logical/text facts separately. Aggregate modifier identity and unsupported side-specific source tokens are explicit. Binding output/priority/context-independence and ActionId ownership are unchanged. |
| Existing RawInputEvent/FusedInputEvent/KeyEvent lineage; bounded supplement to draft 033 §§4,7–9 | K4/K7 require original source association, physical/logical separation and ingress Instant preservation. No adoption of 033's unrelated gesture/sequence design or timestamp-clamping shorthand. |
| 361 §§1–3, especially INV-361-OUTPUT/LATEST/ABSENCE/AUTHORITY | P1 adds the timed present-call fact. P2 is one explicit opt-in, one-shot exact-result exception to latest-only observation, with no history queue, callbacks or physical visibility claim. P3 leaves semantic candidate association to the application. |

No amendment to 020/020A ActionId identity, draft 018 screen precedence, draft
041 animation clock, draft 319, proposed 093 or proposed 363/364 is adopted by
reference. The live Shift+F/F9/F12 implementation shortcuts supply no independent
Hard-reservation authority. Constitution 002/002A remains unchanged: all facts
and mutations stay in their existing owners/phases, with no semantic re-entry
from presentation or asynchronous code.

## 9. JC Acceptance Decisions Still Required

JC must explicitly accept this proposed contract and its scoped owner amendments
before runtime implementation. In particular, acceptance must cover:

1. Captured input precedence over clipboard/fullscreen/diagnostic/app shortcuts,
   with Escape reserved and general Tab consumption allowed as specified.
2. Physical versus logical binding basis in existing framework values, aggregate
   modifier support, and explicit non-admission of side-specific chord tokens.
3. The labelled **present-call-proxy-to-framework-ingress** metric and bounded
   one-shot receipt. If physical cue visibility/onset is required instead, this
   contract is insufficient; that needs separate platform/display authority.

No implementation/native proof is claimed at this revision. Consumer research
and draft application policy do not constitute Halcyon adoption, and this
framework proposal does not accept a product's cue/attempt policy.
