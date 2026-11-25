# SPEC-HAL-383 - TextField Editor-State Handoff for Retained Screen Rebuild

**Date:** 2026-11-12
**Status:** S1A/S1B BLOCKED / NO CODE CHANGE; S1C READ-ONLY SUCCESS; S1D IMPLEMENTED and independently ACCEPTED for the section 9 source, public API and headless ownership/phase gates only. Physical macOS input, application screen navigation and real backend acceptance remain UNVERIFIED. Earlier constructor-time and Layout-time publication designs stay forbidden under SPEC-HAL-002/002A. Sections 1-8 retain that historical design and investigation context; section 9 owns the accepted phase-qualified contract. No general editor service or extra frame phase is admitted.
**Authority:** JC's 2026-11-12 direction to preserve native editor state across retained screen navigation and rebuild.
**Governing:** Halcyon Constitution SPEC-HAL-002 and 002A, SPEC-HAL-052 editing positions, SPEC-HAL-101 TextField, SPEC-HAL-105 state unification and the existing Halcyon Screen lifecycle. No amendment to the Constitution is selected or required.
**Originating requirement:** Preserve the exact native input selection across navigation. Framework proof and consumer native acceptance remain separate.

## 1. Specific observable obstacle and result

The originating native input had to preserve exact unsent UTF-8 text, cursor and directional selection while navigating to another Screen and returning. Halcyon ScreenStack retained the old Screen object, but AppRoot::rebuild_active_view removed its WidgetTree subtree and constructed a new TextField. At the original investigation, TextField::text was readable; selection and cursor were not publicly readable or restorable, and TextField::set_text collapsed selection at the end. Retaining only a FormBinder string could not prove editor-state preservation.

This S1 lets a trusted consumer capture **pure editor-value data** from its currently mounted TextField through an ordinary read-only WidgetTree borrow after input settlement, and use that data to configure a fresh TextField before its new WidgetTree mount. The consumer never owns, retains, swaps or borrows a live widget across phases. In particular, this does not change ScreenStack, Focus, WidgetTree, input routing or a producer's policy.

## 2. Exact public API and state semantics

In existing `src/widgets/text_field.rs` add an **opaque, cloneable value type**:

```rust
pub struct TextFieldEditHandoff {
    text: String,
    cursor: crate::core::editing::TextCursor,
    selection: crate::core::editing::Selection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffError {
    CompositionActive,
    InvalidBoundary,
    LimitExceeded,
    InputRejected,
}
impl TextFieldEditHandoff {
    pub fn text(&self) -> &str;
    pub fn cursor_byte(&self) -> usize;
    pub fn selection_anchor_byte(&self) -> usize;
    pub fn selection_head_byte(&self) -> usize;
}
impl TextField {
    pub fn capture_edit_handoff(&self) -> Result<TextFieldEditHandoff, TextFieldHandoffError>;
    pub fn restore_edit_handoff(
        &mut self,
        handoff: &TextFieldEditHandoff,
    ) -> Result<(), TextFieldHandoffError>;
}
```

The value's fields remain private; no caller constructor, writable direct state, raw TextFieldState, ptr/ID/pointer, callback, raw cache, focus/capture or IME token. It holds independently owned UTF-8 bytes plus the actual logical cursor position/affinity and exact **ordered** anchor/head. If implementing Debug for the handoff, redact its text and positions rather than expose potentially private draft content. No serde, persistence or network format.

Capture is read-only, has no layout, focus, widget or domain effect, and refuses `CompositionActive` while live IME preedit is unsettled. It must reflect the native edited state, including reverse selections. It is not allowed to manufacture a collapsed caret or commit an IME preedit.

Restore is a configuration operation on a **fresh, unmounted TextField** during construction, never a way to mutate a currently mounted widget from an arbitrary callback. Verify the entire handoff before mutation: all three byte offsets lie in 0..=text.len(), on UTF-8 boundaries and on the currently representable legal editing cluster boundaries; the text obeys the actual target TextField max_len_bytes and input filter. If any check fails, return its exact typed error, change no document/selection/cursor, do not clamp, truncate, normalize, reinterpret or guess a caret. If the target is composing, refuse `CompositionActive`. If a full cluster legality check needs an unavailable text-engine layout at constructor time, do NOT use a char-boundary check as a false cluster guarantee: inspect the actual existing SPEC-HAL-052 and editing substrate, then return a precise BLOCKED finding instead of implementing a weaker silently accepted API.

On admitted restore set exact text, cursor including stable affinity, ordered selection and valid editing state; mark text/layout/caret caches dirty for normal owning Halcyon phases. Reset ephemeral drag and visual animation state; do not regrant focus or capture, issue callbacks, replay typing, create undo entries or change unrelated policies. Subsequent layout must bring the caret into the current viewport under existing viewport logic, without blindly restoring stale geometry.

This is **not** full undo-history serialization, across-process restart, IME preedit preservation or a security boundary. The source state held by an application Screen is transient and may be dropped only under its own user-authorized lifetime. A value handoff does not make any source backend or input operation authorized.

## 3. Required event/lifecycle boundary for the first consumer

Existing `src/core/app.rs` `flush_pending_actions_inner` provides ScreenContext.tree=Some(tree) to a Screen's `flush_pending_actions` after canonical input routing. The actual app loop runs this flush **before** `process_nav_queue` and `rebuild_active_view` (`src/core/app.rs:7378-7421`). In that post-route callback, the application Screen can read the **exact mounted NodeId** TextField through `tree.node(id).widget().as_any().downcast_ref::<TextField>()`, capture the owned handoff, and store only the value in its own Screen state. It then queues navigation by `ScreenContext::nav_push_screen`.

When the screen is revealed after `nav_pop_screen`, create a fresh TextField and restore the saved handoff before mounting. Both sides preserve the existing WidgetTree as sole widget owner. The existing screen/on_leave callback has tree=None, so do not require it to access the retiring TextField. On an attempted navigation action, if capture is refused, keep the original screen/widget mounted and report a human-visible refusal; do not navigate and silently drop the selection.

If any other native rebuild can invalidate the mounted editor without the pre-removal settled capture, the consumer must not claim complete native editor-state preservation. The Halcyon S1 proof should establish that the read-only post-route snapshot is fresh before ordinary Screen route/rebuild and identify any remaining gap in the active IME or native resize path. This specification does not authorize a new post-frame queue or a global state mirror merely to avoid such evidence.

## 4. Invariants and forbidden implementations

**INV-HAL383-WIDGET-OWNER:** WidgetTree remains sole owner of TextField instances. Handoff is only a detached pure value, never a widget lease or a mutable reference.

**INV-HAL383-EXACT:** Accepted capture -> new-widget restore yields byte-identical committed text and cursor/selection anchor/head, including non-ASCII and reversed selection, without normalization or clamping.

**INV-HAL383-ATOMIC:** Validation precedes any restore; invalid text/boundaries/constraints refuse without partial mutation.

**INV-HAL383-LIFETIME:** Focus, capture, IME preedit, hover and geometry are engine-owned, not transferable app authority; no hidden persistence or cross-owner grant.

**INV-HAL383-SINGLE-PATH:** Production public API is ordinarily callable without test-harness, compile feature, opt-in mode, hidden flag or environment dependency.

**FORBID-HAL383-DIRECT-STATE:** No public TextFieldState mutation, arbitrary WidgetTree::take_widget/restore_widget, mem::replace of mounted downcast, second TextField ownership, or app-owned input router.

**FORBID-HAL383-RETENTION-CLAIM:** Merely retaining a screen value, text string or synthetic frame does not prove exact native selection; real input and returned editor state need focused evidence.

## 5. Source and implementation scope

Current source anchors (requalify before editing):
- `src/core/app.rs:9360-9395,9431-9433,9677-9693`: screen retained but subtree rebuilt.
- `src/core/app.rs:10322-10360,7378-7421`: genuine read-only post-route tree access and ordering.
- `src/widgets/text_field.rs:172-175,2519-2532,2555-2564`: closed state, text only public, set_text resets selection.
- `src/widgets/text_field.rs:2628-2635`: existing `EditSnapshot` capture internally; this public handoff is **not** a grant to expose the mutable undo/history owner.
- `src/core/editing.rs:47-55,188-239`: byte cursor, affinity, directional selection.
- `src/core/widget_tree.rs:3292-3423`: layout-private take/restore and widget-replacement semantics, not consumer state extraction.

S1 is owned only by `src/widgets/text_field.rs`, the existing relevant widget/interaction owner test surface, and the minimum public black-box TextField proof in Halcyon's existing `tests/` if distinct public compilation requires it. Do not touch `src/core/app.rs`, WidgetTree, ScreenStack, original SPEC-HAL-002/052/101/105, Halcyon renderer/layout/texture/animation, Cargo/feature/dependency files, consumer or backend repositories, or concurrently modified source. No new general editor service, binding transport, host window or persisted state.

Do not use source-guard bypass or directly mutate private cursor/selection from tests as a substitute for public API proof. The test harness can assist internal negative fixtures but the acceptance must also exercise the **ordinary production public method** from a consumer.

## 6. Acceptance and report

Required proof at owner-correct scope:
1. Real editable TextField with multibyte Unicode and a noncollapsed reversed selection yields exact text, cursor and anchor/head after capture and fresh unmounted restore; another keyboard edit continues from the correct selection. Repeat with collapsed middle-of-text caret.
2. Focus loss, resize/rebuild, selected screen navigation and re-entry where emulated by the actual documented Screen lifecycle preserve values without outside-tree widget references; distinguish native input not physically observed from source/test proof.
3. Active IME composition fails closed with no preedit conversion or discarded acknowledged draft; incompatible max text/filters and invalid original boundary reject atomically where representable. No lossy fallback.
4. Existing TextField ordinary typing/selection/caret/undo, native focus/capture and controls remain unchanged; test-specific APIs cannot masquerade as production.
5. Focused Halcyon package check + tests via Grip, exact owned diff inspection, no collateral source change. Avoid broad unrelated test replay.

This S1 does not establish native application navigation or real backend behavior. Exact source and test evidence requires independent framework review; consumer acceptance remains dependent on proof of the missing editor-state owner contract.

## 7. S1B corrective contract after exact shaping-evidence blocker - 2026-11-12

### 7.1 Independent review and supersession

The original constructor-time handoff attempt returned **BLOCKED / NO CODE CHANGE**.
Independent review re-read `src/widgets/text_field.rs` SHA-256
`7499f1ecf45747e2fe466aff499dcdbb9dea6ab799733d4ce984985db81f8c10`,
`src/widgets/text_layout_cache.rs` SHA-256
`71b8492eabff02dfc92d124301195d3650d74c3295a4d1fac6d368beb54cb960`,
and the existing SPEC-HAL-052 cluster authority. A fresh TextField is
unmounted, and TextLayoutCache starts empty/stale. Target legal caret
positions depend on real TextEngine shaping under the actual current
font, width and theme, available in that design only during canonical
`TextField::layout()`. An unmounted, context-free direct
`restore_edit_handoff(&mut self, &value) -> Result<(), Error>`
**cannot truthfully certify final restoration**.

This adopted S1B correction **supersedes the direct immediate restore
signature and constructor-time target-cluster validation of section 2,
the immediate restore timing in section 3, and the affected S1 proof
language in section 6**. The original failed S1A text is preserved
above as dated history, not a second production API. S1B alone is the
selected bounded implementation. Constitution 002, exact TextField
editing and WidgetTree ownership are unchanged.

### 7.2 Two-phase contract and exact public surface

The new production value `TextFieldEditHandoff` and its read-only
accessors from section 2 remain. Only a genuine TextField can capture
one; no public writable constructor or direct-state access is exposed.

Replace the impossible immediate restore with this exact staged form:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffError {
    CompositionActive,
    InvalidBoundary,
    LimitExceeded,
    InputRejected,
    ShapingUnavailable,
    UnsupportedTarget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffStatus {
    NotRequested,
    Pending,
    Applied,
    Rejected(TextFieldHandoffError),
}
impl TextField {
    pub fn capture_edit_handoff(
        &self,
    ) -> Result<TextFieldEditHandoff, TextFieldHandoffError>;
    pub fn try_stage_edit_handoff(
        &mut self,
        handoff: &TextFieldEditHandoff,
    ) -> Result<(), TextFieldHandoffError>;
    pub fn edit_handoff_status(&self) -> TextFieldHandoffStatus;
}
```

`try_stage_edit_handoff` is permitted **only on a fresh unmounted
ordinary single-line, nonpassword, noncomposite TextField**. The
consumer creates and configures that new widget, then stages its
previously captured value **before mounting**. Any target already
mounted, edited, holding a pending handoff, configured to perform
automatic start-captured selection, or incompatible with plain
TextField semantics returns `UnsupportedTarget` with no mutation.
The operation prechecks target byte cap, input filter, absence of
active IME, and all three offsets for bounds and UTF-8 scalar
boundaries. It clones the input into private **Pending** staging
storage but does **not** mutate the active document, cursor, selection,
undo, focus/capture, layout cache or ordinary callbacks.
Returning `Ok(())` means **staged only**, never restored.
The screen retains its independent original handoff value and
does not discard it on this receipt.

The first actual non-measurement `TextField::layout()` computes the
target typography and text width from the same `LayoutContext` and
`TextEngine` as ordinary TextField layout. It builds a **temporary
candidate TextLayoutCache** from the staged text using that very
context, never a test-only injected map, source-widget cache,
synthetic font choice or text-only boundary heuristic. Require
genuine shaping evidence; `TextEngine::shaper() == None`, an
unusable or incomplete candidate, and unknown legality return
`ShapingUnavailable` (no inferred success from fallback empty
lines). Validate cursor position and **ordered** selection anchor
and head against exact legal `ClusterBoundary.byte_start` values
of that candidate, with end-of-text sentinel and full bounds.
A correct UTF-8 or grapheme boundary that is not a target shaping
boundary is `InvalidBoundary`. No rounding, snapping or clamping
is admissible.

Only after **all** target/source/constraint checks pass may that
same owning `TextField::layout()` apply original text, complete
cursor (including affinity) and directional selection as **one
accepted publication** to its widget-owned state; requalify/rebuild
its native cache as needed under actual document version, then
continue the existing layout, caret visibility and paint path.
Reset only ephemeral drag/blink/caret interpolation (no undo/history
entry, callback, IME commit, focus change or input replay).
Set `Applied` only after this exact result. A failure leaves
active editor state unchanged and status `Rejected(cause)`; no
partial text, input, focus or layout-state acceptance may be
projected. Status remains observable read-only on the mounted
widget after that layout. Repeating layout must not apply twice.
No unvalidated pending value may be painted or edited as active
text, and a nonmeasurement Layout with a pending value must
produce exactly one terminal status before its first Paint.

The handoff does **not** transfer active composition, undo history,
focus, capture or source layout geometry. Active IME capture
refuses; a mere `Screen` lifetime is not editor-state survival.
Retaining the original user draft and selection in the application
Screen is necessary until actual `Applied` is observed. On
`Rejected`, the original value remains available for an explicit
human-visible refusal and later lawful recovery, never replaced
by blank success or a silently collapsed selection.

### 7.3 Phase proof, unchanged authorities and exact test gate

Source-backed ordering: `Screen::flush_pending_actions` receives
`ScreenContext.tree=Some(tree)` after input settlement and before
`process_nav_queue` and `rebuild_active_view` in
`src/core/app.rs:7378-7421,10322-10360`. It may capture a mounted
source `TextField` read-only and then request screen navigation.
The old Screen retains the pure snapshot only, and the new
TextField is constructed with private Pending staging on return.
Actual `TextField::layout` uses target
`cx.theme.typography.input_font_size`,
`layout_width_for_policy`, `cx.text_engine` and
`TextLayoutCache::rebuild` at
`src/widgets/text_field.rs:3334-3395`. No AppRoot, WidgetTree,
ScreenStack or TextEngine API change is authorized here.

S1B source owner remains `src/widgets/text_field.rs` and
focused existing TextField/public-consumer tests only. No other
Halcyon source, user data, feature flag or other workspace changes.
Do not change existing editing, font, input event, focus/capture,
layout phase or error semantics for ordinary TextField uses.

Minimum proofs through the ordinary public API:
1. Real shaped Unicode text with an admitted collapsed middle caret
   and reverse noncollapsed selection: capture, stage, observe
   `Pending` before first real layout, `Applied` after actual
   target layout, and exact text/cursor affinity/ordered anchor/head
   and continued input afterward.
2. A UTF-8-valid offset that is **not** a target shaping-cluster
   boundary yields `Rejected(InvalidBoundary)`, no partial
   document/cursor/selection effect; a no-shaper backend yields
   `Rejected(ShapingUnavailable)`, not synthesized proof.
3. Invalid offsets, target length and target input filter fail
   at staging with no mutation. Active IME capture refuses without
   swallowing live preedit. Nonordinary/auto-start target refuses.
4. Ordinary TextField layout/typing/selection/undo and its existing
   native lifecycle remain unchanged. No duplicate application on
   later layout, no data/pointer leak through Debug or output.
5. Explicit public consumer test against real non-test API and
   engine-owned layout proof. Synthetic harness-only cluster
   injection does not count. Report any unproven physical macOS
   input or source-level screen-navigation coupling honestly.

If the actual widget/lifecycle cannot support the Pending -> Applied
or Rejected transition under these exact rules without weakening
SPEC-HAL-002/052 or changing an out-of-scope owner, stop and
return precise **BLOCKED** with source lines. Do not try another
unadmitted signature or fake success.

**S1B disposition:** this correction was selected as a bounded prerequisite to
native consumer editor-state preservation under JC's 2026-11-12 direction. It
did not approve unrelated framework changes. Consumer acceptance remained
blocked until independent public-boundary proof; the later S1B rejection below
remains part of the historical record.

## 8. S1B phase-authority rejection and held next implementation - 2026-11-12

The shaping-qualified Layout-time handoff attempt returned
**BLOCKED / NO CODE CHANGE**. Independent review verified the governing conflict:

- S1B correctly recognized that real TextField Layout has the
  target TextEngine, theme/width and shaping-cluster evidence.
- S1B nevertheless required `TextField::layout` to publish the
  original document, cursor and selection directly into the
  canonical editing state after validation.
- Supreme `SPEC-HAL-002` Pillar B, section 4.2 and section 5
  reserve editor/selection mutation to the deferred mutation
  phase and forbid canonical state mutation in Layout.
  Layout authority under SPEC-HAL-141 covers geometry; it is
  not a waiver for changing the document or selection.
- Existing `start_captured` source behavior is not legal
  authority to introduce an additional Layout editor mutation.

Therefore section 7 S1B's **Layout-time application**, and any
`Pending -> Applied` or `Pending -> Rejected` transition that
changes canonical editor state in Layout, are **inactive and
forbidden**. The older S1A direct restore is likewise inactive.
Neither unit created working source or acceptance proof.

The useful design separation is not itself an adopted
implementation: **target shaping evidence is layout-owned,
while canonical document/cursor/selection publication is
deferred-owner work**. A successor contract must establish
the exact existing or explicitly selected phase-safe
layout-evidence handoff, identity/revision check, wake/
frame scheduling and validated atomic publication. It may
not assume a natural second frame, relabel editor state
as geometry, borrow a live widget across phases, use a
test-only cluster map, or relax UTF-8/shaping/IME requirements.

A read-only investigation was commissioned to identify an existing phase-correct
publication owner without inventing another API. It permitted no source or
documentation change and selected no architecture. At this historical entry,
delivery was confirmed but execution had not been observed. There was **no active
S1C implementation contract**; native consumer acceptance remained blocked on a
phase-correct owner contract and public proof.

## 9. S1D adopted: single-cut phase-qualified TextField handoff - 2026-11-12

### 9.1 Selection and exact supersession

Independent review accepted the successful **read-only** existing-owner
investigation against Halcyon source, supreme SPEC-HAL-002/002A, accepted
SPEC-HAL-052 and SPEC-HAL-354 section 4.1. S1C performed no source changes.
Its additional fact was that ordinary single-line
`TextField::layout_width_for_policy` selects the existing constant
`SINGLE_LINE_LAYOUT_WIDTH = 100_000.0`. The visible widget viewport width
remains independently Layout-owned; it does not alter that single-line shaping
width. The genuine target text engine and settled current Theme are
simultaneously available at Halcyon's **existing pre-Layout deferred mutation
owner cut**.

This section is the only **ADOPTED / ACTIVE / BINDING** HAL383
implementation scope, S1D. It supersedes S1A's context-free immediate
restore and S1B's canonical editor mutation in Layout. Sections 2/3/6
and 7 remain historical evidence where their former phases or public
signatures conflict with this section. No constitutional amendment,
new global phase or alternate mode is selected. The exact user
result is unchanged: byte-exact pending question, native cursor and
ordered selection survive Screen navigation and ordinary rebuild.

### 9.2 Authoritative same-frame phase cut

Both current application loops already perform navigation, screen
widget-subtree rebuilding and then a narrowly authorized deferred
owner cut **before** actual Layout/scene/Paint:

* Native Mac: `src/core/app.rs:18146-18149` screen post-route flush
  supplies read-only mounted input state; `18410` processes navigation;
  `18459` rebuilds; `18614-18618` settles frame Theme;
  `18626-18635` runs engine deferred owner operations with the
  same `renderer.text_engine_mut()` that the subsequent Layout uses.
* UI/test run: `src/core/app.rs:7378-7390` flush/navigation,
  `7421` rebuild, `7549-7552` settles frame Theme and the
  existing pre-Layout owner cut precedes `build_scene_reusing`
  at `7620`. Add the same narrow operation after theme settlement
  and before normal layout in both real loops.
* Binding `SPEC-HAL-354 section 4.1` already qualifies
  engine-owned deferred/construction work and exact
  owner-selected publication without inventing a second Widget
  Update or new frame phase. The existing post-rebuild,
  pre-Layout `process_composite_choice_work` is a phase-order
  precedent, **not** authority to invoke composite-editor APIs
  on an ordinary standalone TextField.

This S1D selects a **separate TextField-owned operation** in that
existing engine cut. It never runs in `TextField::layout`, Paint,
input event routing, rendering preparation, or asynchronous thread.
It never needs an additional frame: when a fresh TextField was
mounted in this frame, the same frame's upcoming Layout sees the
fully validated applied state. No UiWakeHandle, timer, second
scheduler, busy loop, later Layout-result reconciliation, or
unverified promise of another frame is required.

### 9.3 Public exact type and operation surface

Use the S1B **public** names, with the S1D phase semantics below:

```rust
pub struct TextFieldEditHandoff {
    text: String,
    cursor: crate::core::editing::TextCursor,
    selection: crate::core::editing::Selection,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffError {
    CompositionActive,
    InvalidBoundary,
    LimitExceeded,
    InputRejected,
    ShapingUnavailable,
    UnsupportedTarget,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextFieldHandoffStatus {
    NotRequested,
    Pending,
    Applied,
    Rejected(TextFieldHandoffError),
}
impl TextFieldEditHandoff {
    pub fn text(&self) -> &str;
    pub fn cursor_byte(&self) -> usize;
    pub fn selection_anchor_byte(&self) -> usize;
    pub fn selection_head_byte(&self) -> usize;
}
impl TextField {
    pub fn capture_edit_handoff(&self)
        -> Result<TextFieldEditHandoff, TextFieldHandoffError>;
    pub fn try_stage_edit_handoff(&mut self, handoff: &TextFieldEditHandoff)
        -> Result<(), TextFieldHandoffError>;
    pub fn edit_handoff_status(&self) -> TextFieldHandoffStatus;
}
```

`TextFieldEditHandoff` is opaque and cloneable, with owned UTF-8 text
plus the **full original** TextCursor (including affinity and
preferred_x) and directional Selection (anchor/head). There is no
public arbitrary constructor, mutable internal editor state,
TextField widget lease, persistent reference, raw layout cache or
capability to select any other UI owner. Its Debug must redact
private content and offsets. Capture is read-only and refuses
active IME composition; it never guesses a preedit commitment.

`try_stage_edit_handoff` applies only to a newly constructed,
unmounted, ordinary **single-line, nonpassword, noncomposite**
TextField with no prior text/input or pending handoff. It rejects
automatic `start_captured`, `select_all_on_focus` or
`select_all_on_capture` that would overwrite saved direction.
Before storing the copied candidate, verify UTF-8 byte bounds and
scalar boundaries of cursor/anchor/head, the target's byte limit
and input filter, supported target profile and no active IME.
Validation is atomic and fail-closed. The only successful staging
side effect is retaining an **owned inert pending value** in that
fresh widget. `Ok(())` means `Pending`, never `Applied`.
It does not publish a document, cursor, selection, undo/history,
focus or capture, nor emit callbacks. The original application Screen
keeps its own detached snapshot until it can observe the actual
owner's `Applied` terminal status.

### 9.4 Engine-owned qualification and publication, not a new phase

On mount into the sole WidgetTree, register only TextFields that
actually have a pending HAL383 handoff in a bounded tree-owned
one-shot index. Cover ordinary `add_child`, `create_root`,
and any `replace_widget_only` path that can install a staged
TextField. The index contains **exact owner and concrete mount
identity** (existing `WidgetMountToken` when applicable), not
an ID-only alias or a widget reference. Retiring/replacing a node
invalidates its old index entry; processing always revalidates
live mount, current concrete TextField and the matching original
pending instance. Do not scan the complete WidgetTree on every
ordinary frame to find candidates. Do not turn this index into
a new authority registry or persistent store.

At the existing post-rebuild **deferred mutation cutoff**, engine
supplies `&mut WidgetTree`, the actual current `&mut dyn TextEngine`
and the **same settled Theme** chosen for that frame's upcoming
TextField Layout. The tree owner consumes only exact live pending
entries and calls the TextField's crate-private, owner-specific
qualification/publication method under one scoped mutable widget
borrow. No screen/controller/user callback may directly invoke
this internal method. The pending snapshot's identity and target
configuration must not change between qualification and
publication, and there is no asynchronous handoff.

For each candidate, use the real TextEngine's `shaper()` and the
Theme's `typography.input_font_size` in a **temporary
TextLayoutCache**, with exactly the existing normal single-line
shaping width `100_000.0`, and existing normal text/paragraph
shaping pipeline. Require actual glyph/run evidence or a
source-justified complete empty-text case; a no-shaper fallback
`Line::empty`, stale cache, missing sentinel or incoherent
cluster offsets fails `ShapingUnavailable`. Verify each original
cursor/ordered anchor/head byte offset exists among the
candidate's real legal `ClusterBoundary.byte_start` values,
including the genuine end sentinel. UTF-8 scalar positions
that are not target shaping boundaries fail `InvalidBoundary`.
No rounding, clamping, altered prompt, normalization, test-only
boundary injector or synthetic font/width is permitted.

Only when **all** validations succeed does the same TextField
owner atomically install exact document bytes, full cursor and
ordered selection, retire the one pending value and publish
`Applied`. Invalidate its genuine document/layout/paint caches
for the upcoming ordinary Layout. Keep ordinary UndoHistory
new and empty, preserve focus/capture/IME authority, do not
fire `on_change` or submit, replay input, change host action
semantics or leave partial effects. A failed candidate must
not modify document/cursor/selection: discard the *target's*
pending candidate, publish exact `Rejected(error)`, and keep
the *screen's independent* original snapshot for recovery.
Do not reattempt without an explicit new owner request.

The engine consumes this registry **once** before any Layout or
first Paint involving a newly mounted staged TextField. No
unvalidated or rejected candidate content becomes active text.
A matched pending candidate must settle `Applied` or
`Rejected` before first presentation; otherwise fail the
specific admission, not the whole app or independent widgets.
A normal TextField with no candidate is byte-for-byte behaviorally
unchanged. A resize/theme change after the successful admission
uses the ordinary editing/layout owners; if target font/shaping
can no longer certify the retained positions, do not silently
snap them or rewrite the saved selection under a fake exactness
claim. Report the actual first such failing case for a separate
owner decision; this S1D does not add a global editing repair.

### 9.5 Consumer sequence and tests

The application Screen captures its real native TextField's pure snapshot from
`Screen::flush_pending_actions` after routing and before any navigation or tree
rebuild; a rejected active IME snapshot blocks the affected navigation and
preserves the mounted input. It also refreshes this pure value before any normal
screen rebuild that would retire the mounted field, not only on user-initiated
navigation. Returning to the original screen builds a **fresh** TextField staged
with that owned snapshot. The engine's real pre-Layout deferred owner cut applies
it before the same frame's input control is painted. At the next normal screen
action/flush the consumer reads the mounted `TextField::edit_handoff_status()`
to distinguish true `Applied` from typed refusal; it never treats `Pending`
as success. On refusal it keeps the original value, displays the typed error
using the existing screen state and does not silently discard the draft or
create a fake owner. Separate application work ownership and background execution
begin only through their explicit application action; editor handoff does not
activate them.

Required **Halcyon S1D proof**:

1. Public non-test-harness capture and staging API compiles
   in an independent consumer; real shaped Unicode text,
   collapsed middle caret with affinity and reverse selection
   survive a fresh WidgetTree mount, **same frame pre-Layout
   deferred settlement**, actual Layout and next keyboard edit.
2. Real target shaper/same-frame Theme and fixed single-line
   100000 shaping width are used. Prove UTF-8-valid yet
   *shaping-invalid* offset rejection with no partial mutation;
   no-shaper and incomplete map refuse, never fabricate proof.
3. Source or target active IME, incompatible max bytes,
   restrictive filter, unsupported target/autocapture and
   bad offset each fail before a wrong publication. A
   destroyed/replaced or foreign mount cannot receive a
   former owner's pending value. Repeated owner cuts settle
   at most once and no idle per-frame tree sweep occurs.
4. Verify **both** run_ui_loop and actual native App::run
   have this same pre-Layout engine cut, with source and
   focused frame/WidgetTree evidence of first-paint ordering.
   Headless frame output alone is not physical macOS input
   evidence. Document exactly what the native host proved.
5. Existing TextField editing/undo/selection/input, frame
   structure, application work, active appearance/render
   modifications and original owner tests remain intact.

Implementation scope is ONLY `src/widgets/text_field.rs`,
`src/core/widget_tree.rs`, the minimum exactly two owner-cut
call sites in `src/core/app.rs`, and focused existing
Halcyon tests/one distinct public consumer proof if required.
The two core files contained unrelated active work at adoption.
Preserve foreign edits; a same-region conflict is not permission
to overwrite or relocate unrelated logic. No changes to the
Constitution, ordinary TextField/Screen/Stage contracts,
`TextEngine`, `TextLayoutCache`, `src/core/editing.rs`,
consumer/backend repositories, production resources,
Cargo dependencies/flags or filesystem data.

**S1D status:** the preceding S1A/S1B code mandates remain explicitly rejected.
S1C was read-only. S1D is the first authorized phase-qualified production
attempt. Delivery alone is not acceptance: exact terminal source and
verification evidence requires independent review before dependent native
consumer acceptance can resume.

## 10. Independent S1D source/headless acceptance - 2026-11-12

The phase-qualified deferred handoff implementation returned **SUCCESS** with
scoped source, owner, public-consumer and headless proof, explicitly without
physical macOS input or a native consumer result. Independent review verified
the exact reported source identities and reran the relevant package
qualification. The implemented public `TextFieldEditHandoff`,
`capture_edit_handoff`, `try_stage_edit_handoff` and
`edit_handoff_status` are present without test-only production features.
A fresh unmounted field stages inert Pending; canonical text/cursor/selection
publication occurs only at the existing owner-held deferred cut with real
TextEngine and settled Theme, never during Layout or Paint.

Accepted source identities, SHA-256:

* `src/widgets/text_field.rs`:
  `2c517a527ea5c3494a9aa71b4d86e90cff8779e145af629d5fcb140dcb87135e`.
* `src/core/widget_tree.rs`:
  `e352bec60055fdd837de506b8d0b418176f9e0ef7e4da33842015b770070b327`.
* `src/core/app.rs`:
  `33abbb389051c0ab30aa0f79d8553660482fe26a216109106e8c7ef5f28b5a1f`.
* `tests/text_field_handoff_public.rs`:
  `53a3343cdb5d6c7f624ea8854657ce6f29d4705bcb5594699cfc32b68588246b`.

The original WidgetTree source line 673 one-shot index
checks concrete mount and liveness, not merely matching
numeric IDs. Both ordinary UI and native App runtime
call `process_text_field_handoffs` once within the
current deferred owner cut at respectively
`src/core/app.rs:7568` and `18959`, before first
actual Layout/scene. The exact private candidate uses
target font, 100000-width single-line shaping,
checks the complete cluster evidence, and atomically
accepts full cursor/affinity/selection or returns a typed
refusal. Existing application work and Stage ownership are not
modified by this change.

The executor's scoped verification recorded:
`cargo test -p halcyon --lib edit_handoff` 8 passed;
`--test text_field_handoff_public` 1 passed;
`--lib ui_wake` 7 passed; and
`--lib core::widget_tree::` 26 passed;
all zero failures. A normal production rlib
compiled the public consumer without a test-harness.
Independent Grip verification additionally
recorded `verify package halcyon test edit_handoff
refresh check timeout 240`: check exit 0,
zero errors, 9 matching tests passed; and
`verify package halcyon test ui_wake timeout 240`:
9 matching tests passed, zero failed. Full suite
success is NOT claimed.

The broader pre-existing TextField suite has three
smooth-caret test failures, proved identical in the
executor's controlled before/after baseline and
not modified or claimed repaired by S1D. HAL383
does not add broad animation changes to address
unrelated tests. No physical OS pointer, keyboard,
active IME or native screen-navigation roundtrip was
executed by this unit; its headless retained
first-Paint evidence must not be relabeled physical
acceptance.

**Disposition:** independently ACCEPT S1D only as the public/owner/headless
prerequisite for separately scoped native consumer acceptance. Native
editor-state preservation and real backend text generation are not established
merely because this prerequisite is accepted. No other Halcyon API, backend,
client-data, memory or cross-project grant follows from this evidence.

## 11. Shared existing handoff cut, Markdown-only extension - 2026-11-14

HAL390 section16 authorizes the original lazy concrete-mount handoff index to
carry an explicit owner kind for TextField and MarkdownView. The same native
and UI-loop pre-Layout deferred cut dispatches each one-shot request to its
own widget. This is a mechanical extension of the existing owner index, not
another phase, general state service or public mutation API. Section9's exact
TextField source, target shaping, filtering, atomic publication, errors and
public signatures remain unchanged. Its existing owner/public tests are
required after the shared index extension. No TextField undo transfer, IME
retention or new editor class is selected.
