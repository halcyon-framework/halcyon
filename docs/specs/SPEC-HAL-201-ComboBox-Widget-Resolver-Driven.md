# SPEC-HAL-201: ComboBox Widget (Resolver-Driven, Zero Semantics)

**Status:** Binding
*Touch picker supersession (2026-11-21):* Binding HAL-400 authorizes host-native device choice presentation only for an admitted genuine Touch choice interaction. This changes neither this SPEC's Resolver Selection semantics nor its existing Mouse/Pen/physical-keyboard popup, focus and navigation behavior. Native platform UI reports a qualified OptionId or Cancel; Halcyon exclusively decides acceptance.
**Layer:** Widget / Interaction
**Scope:** ComboBox widget structure, input forwarding, popup lifecycle
**Depends On:** SPEC-HAL-199, SPEC-HAL-002
**Bound By:** SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution), §4.11 (Explicit Commit Authority); SPEC-HAL-199 §6.5–6.9 (Adapter-Owned Search & Commit Policy)
**Non-Goals:** Matching, filtering, ranking, commit inference, normalization, default resolver creation, async data, virtualization, hierarchy

---

## 1. Purpose

ComboBox is a pure interaction shell over a resolver-driven data model.

It renders a control surface, manages a popup, tracks highlight state, and forwards user actions verbatim to a `ResolverHandle`. It never decides what the user meant.

The widget owns presentation and interaction mechanics. The resolver owns commit semantics. This boundary is permanent.

---

## 2. Widget Responsibilities

ComboBox is responsible for exactly the following:

- Render a control surface (current selection label, optional text input field)
- Manage popup open/close state
- Track the currently highlighted option by `OptionId`
- Emit explicit commit actions only (never synthesized, never inferred)
- Forward signals verbatim to the `ResolverHandle`

ComboBox is NOT responsible for:

- Deciding what the user meant
- Matching input text against option labels
- Filtering, ranking, or sorting options
- Normalizing, case-folding, or transforming input
- Choosing between typed input and list selection
- Providing fallback behavior when the resolver rejects

---

## 3. Inputs to Widget

### 3.1 ResolverHandle

The widget holds an opaque `ResolverHandle`. It calls `resolve()` on explicit commit actions and acts on the returned `ResolverDecision`. It never inspects the resolver's internals.

### 3.2 Option List

The widget receives the option list as borrowed `&[OptionEntry<'a>]` from the adapter. The widget renders these options in the popup list. The widget does not own, filter, reorder, or modify this list.

### 3.3 Presentation-Only Policy Flags

The widget may accept flags that affect presentation only:

- Whether the text input field is visible
- Placeholder text
- Display formatting for the selected value

These flags must not affect commit semantics, matching behavior, or resolver input construction.

---

## 4. Signals Forwarded to Resolver

On an explicit commit action, the widget constructs a `ResolverInput` from the current UI state and forwards it to the resolver. The signals are:

| Signal | Type | Source |
|--------|------|--------|
| `input_text` | `Option<&str>` | Current text input buffer (None if no text field) |
| `highlighted_id` | `Option<OptionId>` | Currently highlighted option in popup list |
| `commit_source` | `CommitSource` | `Selection` (click/keyboard select) or `InputConfirm` (Enter on text) |
| `options` | `&[OptionEntry<'_>]` | Full option list (borrowed, adapter-owned) |

### 4.1 Signal Construction Rules

- `input_text` is the verbatim text buffer contents. No trimming, no normalization, no transformation.
- `highlighted_id` is the `OptionId` of the currently highlighted option, or `None` if nothing is highlighted. The widget does not resolve this ID to a label or value.
- `commit_source` is determined by the input event that triggered the commit, not by widget-side analysis of state.
- `options` is the same slice the widget received from the adapter. Passed through, not copied or filtered.

---

## 5. Commit Handling

### 5.1 On ResolverDecision::Commit

The widget emits an application-level event carrying the `CommittedValue`. The popup closes. Focus returns to the control surface. The interaction is complete.

The widget does not inspect, validate, or transform the committed value.

### 5.2 On ResolverDecision::Reject

The widget does nothing. No mutation. No fallback. No retry. No error correction. No alternative suggestion.

The widget MAY surface the rejection reason to the user as passive feedback (e.g., a transient visual indicator). The widget MUST NOT use the rejection to alter its own behavior, retry the commit, or attempt a different commit source.

### 5.3 No Widget-Side Disambiguation

If `input_text` and `highlighted_id` both have values, the widget forwards both signals to the resolver without choosing between them. The resolver decides. The widget never decides.

---

## 6. Popup Lifecycle

### 6.1 Open

The popup opens on explicit user action:

- Click on the control surface
- Keyboard activation (e.g., Alt+Down, Space)

Enter key behavior depends on focus target and popup state:

- **Control surface focused, popup closed:** Enter opens the popup. This is activation, not commit.
- **Text field focused, popup open:** Enter is a commit attempt (`CommitSource::InputConfirm`). Not an open action.
- **Popup list focused, popup open:** Enter on a highlighted option is a commit attempt (`CommitSource::Selection`). Not an open action.

Opening the popup does not constitute a commit or selection.

### 6.2 Close Without Commit

The popup closes without commit on:

- Escape key
- Click outside the popup
- Focus loss

Closing without commit discards popup-transient state only:

- **Highlight state resets.** The currently highlighted `OptionId` in the popup list is cleared.
- **Text buffer is preserved.** If the control has a text input field, its contents survive popup close. The text buffer is control-surface state, not popup-transient state. Discarding user-typed text on mere popup close would violate user expectation without explicit action.

No implicit commit. No auto-accept on blur.

### 6.2.1 Widget-local outside press handoff

The widget-local ComboBox popup is non-modal. Its hit region covers the
popup content, including the scrollbar. On an uncaptured primary mouse
press outside the popup and its trigger, the router first notifies the live
popup owner to close without commit and retires its hit region. The same
press then follows ordinary hit, focus, capture, and delivery to its actual
target. Blank space also dismisses. Dismissal is a lifecycle notification,
and the popup owner never receives a copy of the outside pointer event.

The HAL-127 and HAL-137 outside-dismiss consumption rules still govern
blocking overlay stack entries and modal shielding. The ComboBox handoff
does not deliver through an accepting overlay or change existing capture.

### 6.3 Close With Commit

The popup closes with commit only when the resolver returns `ResolverDecision::Commit`. The widget closes the popup and emits the committed value atomically.

### 6.4 Reopening

Reopening the popup after a commit begins a new interaction. It is not retroactive modification of the previous commit. Highlight and input state reset to initial values.

---

## 7. Explicit Prohibitions

The following widget-side behaviors are permanently forbidden:

### 7.1 No Label Inspection

The widget must not inspect option labels for equality, substring matching, prefix matching, or any form of comparison. Labels are for rendering only.

### 7.2 No String Comparison

The widget must not compare input text against option labels, option values, or any adapter-provided data. All comparison logic belongs to the resolver.

### 7.3 No Intent Guessing

The widget must not infer which option the user "meant" based on partial input, proximity, frequency, recency, or any heuristic. If intent is ambiguous, the resolver decides.

### 7.4 No Commit Synthesis

The widget must not synthesize commits from:

- Focus loss
- Timer expiration
- Popup close
- Input blur
- Partial input state
- "Best guess" selection

All commits originate from explicit user actions routed through the resolver.

### 7.5 No Filtering or Reordering

The widget must not filter, hide, reorder, or modify the option list. The adapter provides the list; the widget renders it verbatim.

### 7.6 Constitutional Enforcement

These prohibitions enforce SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution) and §4.11 (Explicit Commit Authority) within the ComboBox domain.

---

## 8. Failure Modes

All failure modes are handled passively. The widget does not attempt recovery, correction, or alternative action.

### 8.1 Empty Input

User presses Enter with empty text field. Widget constructs `ResolverInput` with `input_text: Some("")` and `commit_source: InputConfirm`. Resolver decides. Widget acts on decision.

### 8.2 No Match

User types text that matches no option. Widget constructs `ResolverInput` with the verbatim text. Resolver decides (reject or accept as custom value per policy). Widget acts on decision.

### 8.3 Resolver Rejection

Resolver returns `ResolverDecision::Reject`. Widget does nothing. No retry. No fallback. No error correction. Optionally surfaces rejection reason as passive visual feedback.

### 8.4 No Highlighted Option

User triggers commit with no option highlighted. Widget constructs `ResolverInput` with `highlighted_id: None`. Resolver decides. Widget acts on decision.

---

## 9. Non-Goals

The following are permanently out of scope for this SPEC:

- "Smart" UX that anticipates user intent
- Implicit commits (auto-accept on blur, timer-based commits)
- Auto-accept on blur or focus loss
- Search logic of any kind (filtering, matching, ranking, fuzzy search)
- Autocomplete behavior
- Multi-column popup layouts
- Hierarchical popup structures
- Async data loading
- Virtualized popup lists

Any of these require a separate SPEC and must not be implemented by extending ComboBox widget logic.

---

## 10. Authority Model

| Concern | Owner |
|---------|-------|
| Selection Value | Adapter / Model |
| Matching / Resolution | Resolver / Adapter |
| Commit Policy | Resolver / Adapter |
| Option List Content | Adapter |
| Popup Open/Close | ComboBox Widget |
| Highlight Tracking | ComboBox Widget |
| Input Text Buffer | WidgetTree-owned TextField child; ComboBox retains only source-qualified owned draft observations. |
| Signal Construction | ComboBox Widget |
| Rendering | ComboBox shell/popup and ordinary engine traversal of the tree-owned TextField child, under their respective owners. |
| Keyboard Routing (popup) | Popup System |

HAL-101 section 2.4 governs the fixed text-entry relation, engine-mediated operation/result ordering and runtime draft API boundary. ComboBox still constructs ResolverInput and owns popup/highlight policy; the resolver/adapter retain all existing semantic authority. Section 6.2's text preservation across close remains binding: retiring delegation or animation does not clear the buffer. A successful resolver command may reset the child only through the qualified engine-owned operation, without replacing a host-held Widget or retaining a child borrow.

The closed host adapter preserves this widget's current command, resolver/value/tag, callback and clear/reset order under HAL-101 section 2.5; ordinary standalone TextField Enter/Cancel shall not replace host policy. Runtime child-dependent API calls require the scoped owner context; no no-context peer replacement or silently queued sync is admitted.

Violations are forbidden.

---

## 11. Summary

ComboBox is an interaction shell. It renders, tracks, and forwards. It never decides.

The resolver is the sole authority on commit meaning. The widget can be audited line-by-line for semantic emptiness. If the widget contains a string comparison, a label inspection, or a conditional based on option content, it is in violation.

---

## 12. Cross-References

See SPEC-HAL-202 for derived composition analysis when ComboBox is embedded as an editor widget inside ListView, TreeView, or PropertyEditor.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.

## 13. JC-scoped per-instance frosted display — 2026-11-17

JC directly requests genuine HAL385 backdrop glass for the existing language
ComboBox. The initial component profile is standalone, text input disabled and
MatchTrigger, with an explicitly selected InputSurfaceAppearance. Other
profiles are not silently admitted. A checked attachment consumes the existing
FrostedGlassAppearance; it changes no resolver, input, layout or semantic value.

Closed presentation requests glass before its foreground. During an open or
closing popup, one aligned joined body is requested in the existing same-surface
overlay scope, before the parent's display label, icon, caret and option text.
The first zero-height opening sample still presents the control. There is no
second glass capture over its foreground and no transfer of a tree-owned editor
child into host paint. Editing-mode attachment is explicitly refused.

Existing surface values remain finite state tints/chrome; the popup's common
tint is applied once, with its highlight composed over it in linear RGBA before
one exterior coverage application. Trigger and row fills share the joined outer
silhouette; text and scrollbar content keep their popup/reveal clip. All ordinary
defaults and unsupported-mode safeguards remain intact.

HAL385's same-surface, exclusive-order, geometry, inherited-clip, budget and typed
renderer-refusal rules remain binding. This profile does not admit another
SurfaceId, generic child layer transport, a new blur pipeline or a consumer
backplate. Existing control-subtree ordering may qualify the source interval.
Source implementation, native pixels, real browser output and JC optical
acceptance are distinct evidence gates.

## 14. Per-instance popup scrollbar display — 2026-11-18

An overflowing popup shows its scrollbar by default (`WhenOverflowing`).
Each instance may opt into `AutoHide` and select finite inactivity hold
and fade durations through `PopupScrollbarInteractionPolicy`. The inclusive
limits are 0–60 seconds for hold and 0–10 seconds for fade. The defaults are
1.5 and 0.3 seconds. `with_popup_scrollbar_policy` changes only the display
mode. The last builder to select a mode determines that mode.

With AutoHide, opening with overflow, gutter hover, actual row scrolling,
and thumb drag restore the current appearance to full configured opacity.
Hover and drag keep it visible. After inactivity the scrollbar fades to
zero, or disappears when the hold ends if the selected fade is zero.

`PopupScrollbarAppearance` separately selects finite opacity factors in
the inclusive range 0–1 for the track, normal thumb, and hovered or pressed
thumb. Each defaults to 1 and multiplies the corresponding theme alpha and
transient fade without changing theme RGB. Geometry remains in
`ScrollbarConfig` and `ScrollbarV::layout`.

HAL-086 and HAL-110 retain scroll and geometry authority. Overflow determines
whether the popup can consume wheel scrolling. Transient scrollbar presentation
determines whether its gutter is reserved for text and whether its published
track and thumb can take a press. At least one part must have positive
effective alpha for the rail to be visible and eligible for a new press.
While hidden the gutter remains available to the rows. AutoHide hover may
observe that gutter to reveal the scrollbar.
The retained overlay hit entry carries the full legal row bounds and the
exact ScrollbarV layout of the current paint sample. During reveal the drawn
track and thumb govern scrollbar presses, including page versus drag. A
hidden scrollbar does not intercept presses. Popup placement and row bounds
continue to follow their existing layout and reveal contract.
Row height, popup allocation, option order, and resolver semantics stay fixed.

## 15. Adapter-backed type-to-select — 2026-11-18

JC admits a per-instance type-to-select callback for the open popup of a
non-editing ComboBox in keyboard Capture. Plain and Shift-modified typing uses
the original routed text event verbatim. The callback borrows this instance's
current ordered option IDs and localized display labels and may return one ID.
The adapter owns prefix matching. ComboBox accepts only an ID present in that
projection, moves the highlight, and brings its row into view.

While this route is active, printable text including j/k/i goes to the adapter.
The paired key event leaves delivery of typable text to the existing router.
Escape retains its normal cancel behavior, and Enter or a row click still
commits only through the existing resolver. A missing or invalid match leaves
the highlight and selection alone. The callback changes neither option order
nor the selected value. Editing mode and instances without the callback retain
their current input behavior.

## 16. Opt-in closed Hover and Focus surface — 2026-11-18

A standalone ComboBox may prefer its configured Hover face when the popup
has settled closed and both Hover and framework Focus are present. The
Focus cue and keyboard authority still follow HAL-328. The preference
requires current owner-qualified facts showing neither keyboard nor pointer
capture. Popup reveal, capture, disabled, and unconfigured controls keep
their existing surface rules. HAL-012's shared material precedence is
unchanged.
