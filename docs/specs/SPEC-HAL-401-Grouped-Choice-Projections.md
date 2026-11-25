# SPEC-HAL-401: Grouped Choice Projections

**Status:** Adopted / Active / Binding. **Date:** 2026-11-22.
**Authority:** JC approved grouped choice presentation and explicitly directed implementation. This SPEC governs reusable Halcyon projection, popup and host-picker behavior. Applications own their option catalogs, ordering, localization and stable value identities.
**Owner:** Existing ComboBox projection, popup and keyboard input, together with the HAL-400 host-choice transport. Applications supply the catalog through the public projection API.
**Depends:** HAL-002 / 002A Constitution, HAL-199/201 Resolver/popup, HAL-112/128/396/397 touch/gesture, HAL-398 browser editing, HAL-400 host-native picker, and PM-002/014/018/023/029. HAL-131A remains locked.

## Scope and verification

- **Objective:** Add group headings to a normal Halcyon ComboBox using the SAME canonical ordered selectable OptionIds and Resolver. Render nonselectable headers in the Halcyon popup for mouse/keyboard; project native HTML optgroups in WASM and explicitly uncommittable section rows in UIKit. Preserve all existing flat ComboBox behavior.
- **Consumer scenario:** A grouped selector presents two ordered groups with independently authored labels. The representative fixture contains 30 and 219 real options. A normal touch opens the host-native picker; selection from either group commits its original OptionId exactly once; cancellation and panning preserve state. Existing flat selectors, mouse navigation, touch input, IME and kinetic scrolling retain their behavior.
- **Boundary:** Halcyon supplies the reusable grouped presentation. Applications supply catalog contents through the public API; they do not implement a second HTML picker.
- **Source baseline at adoption:** src/widgets/combo_box.rs OptionSnapshot stores Vec<(OptionId,String)>; current set_projection is flat; popup scroll, paint and row hit assume every display row corresponds to one option. HAL-201 sends only actual options to Resolver. src/core/event_router/host_choice.rs ChoiceOptions and ChoiceRequest project flat tuples; src/platform/web/choice_picker.rs::make_selector creates one HTML option per tuple; src/platform/native_choice_ios.rs currently returns each picker row as an OptionId. Grouping currently does not exist.
- **Owned files:** existing ComboBox/projection model and tests, existing host-choice projection, Web SELECT builder, UIKit picker delegate, original examples where useful; no new widget subtree or generic list infrastructure.
- **Forbidden:** fake selectable separator, reserved OptionId, adding header to ResolverInput.options, grouping by label guesses, automatic sorting or locale guessing inside ComboBox, index-based commit after a header, duplicate model/host owner, second popup/native select, changes to mouse/keyboard input route, extra feature flag or product HTML control.
- **Acceptance:** Two groups of real options, with visible headings that are never accepted or highlighted as options; ordinary-list performance for the 249-entry fixture; no double selection, stale host revision/capture, keyboard or scroll regressions. Actual iPhone native presentation requires separately observed physical evidence.
- **Scope:** normal source, focused tests, demo/example. No network/persistent state/API/deployment/release side effects.

## Architectural boundaries

- Governing: unchanged Constitution HAL-002/002A and HAL-199/201 OptionId/Resolver exclusivity; HAL-400 already owns one native host picker; this SPEC extends only presentation of the original list.
- Widget semantics: group is optional presentational structure from the adapter. It never becomes a selectable value, index, search result, data owner, or Resolver input.
- Interaction: group headers do not receive hover selection, pointer capture, Tap, keyboard focus or Enter/Space commit. Search/type-to-select operates on real options only.
- FORBID: platform callback modifying widget state, OS choice deciding semantics, independent DOM UI, group-label-to-value inference, hidden mobile flags or synthetic Mouse input.
- Reuse: current ordered projection, popup scroller, type-to-search, current host offer/lease/revision and original composite Resolve Selection, original layout/event/render pipeline.
- Demo: one grouped ComboBox in examples/ in normal API, while existing flat examples retain their exact output and behavior.

## 1. Public projection contract
1. Preserve the existing ComboBox::set_projection(items, highlight_hint, min_visible_rows, max_visible_rows) semantics and every current flat call site unchanged.
2. Introduce an additive grouped projection entry point on ComboBox receiving the same ordered real (OptionId, label) items PLUS an immutable ordered slice of group boundaries. Each boundary supplies an index into the real item list and an already prepared display heading (e.g., OptionGroupBoundary { start_index, title }). The two sequences are installed atomically as one projection revision; never set independent grouping state in a second setter.
3. Preconditions: for a grouped list, at least one real item; first boundary.start_index == 0; strictly increasing start indices; every index < items.len(); no empty group, no empty heading; no duplicate real OptionId. A malformed projection refuses admission without changing the previous complete projection. The existing flat projection remains valid without any boundaries.
4. Boundaries are indexes, not IDs. They represent the beginning of a display group only and are never published as real option rows. Stable OptionId is retained for actual choices and derives from the consumer's current adapter. No header, synthetic placeholder or separator may carry any OptionId.
5. The widget does not infer group membership, mutate item ordering, perform case folding, translate labels, re-sort, choose defaults or interpret input. All grouped metadata and its labels come fully authored by the product adapter.
6. ResolverInput.options remains the original exact ordered slice of REAL OptionEntries, with no headers or groups. The existing model-owned search/projection logic remains. Existing callbacks report only unchanged original committed values.

## 2. Halcyon popup presentation and navigation
1. Build a derived display-row view for grouped projections containing Header(title) or Item(index into the immutable real item list); flat projections retain their existing row enumeration with zero header rows. This view is ephemeral presentation only, never an authoritative source or second list.
2. Popup height, scroll range, scrollbar thumb, popup_scroll_row, visible rows, retained/virtual paint, clip and pointer hit use DISPLAY-ROW indexes for grouped lists. Each Item row maps back to its original real OptionId. The header is painted in a visually secondary style, visually distinct and unselectable; an adjacent divider may be painted under the same popup clip without changing normal popup chrome. The existing row height and scroll logic for flat projections remain unmodified.
3. Pointer down/up/touch hit on Header never begins a choice press, commits, highlights, changes focus, or calls the Resolver. A scroll gesture beginning on a header remains eligible for the normal popup Scroll candidate; it does not click through to the page.
4. Up/Down/End/Home, wheel scroll and type-to-select operate on REAL selectable options, not header rows. Navigation skips headings in both directions and at group boundaries. Keyboard Enter/Space commits only a highlighted real OptionId. Escape cancels as before.
5. Match highlights remain indexed by real-item position, NOT derived display-row position. Existing popup search, projection revisions, list bounds, overlay order, focus and cancellation remain owner-correct. No O(249) work in every pointer move or frame; compute the display-row mapping only on projection change.
6. Preserve every preexisting flat ComboBox's popup behavior, including independently supplied language and profile selectors. No group headings or margins may appear for a flat projection.

## 3. HAL-400 Web native SELECT
1. Extend the existing one browser SELECT transport to receive an immutable grouped presentation from the owner-qualified ChoiceRequest/ChoiceOptions. Render actual HTML optgroup nodes with supplied labels; each member is a real option value mapped to the original OptionId. No fake option or select-element separator.
2. WebKit still opens a true host-native picker on the genuine admitted Touch. Existing touch-down/move/up forwarding and scroll/pan arbitration are unchanged; do not require unsupported showPicker() or synthesize clicks. Native host focus/selection semantics remain delegated only as presentation.
3. Reuse original owner lease, mount and revision. An actual change submits only an OptionId that appeared in the admitted ordered real options. Blur/dismissal, stale projection or a pan cannot commit. Input change/blur must remain idempotent.
4. Browser device presentation of optgroup headers may differ by UA; use native group labeling where supported rather than a custom HTML imitation. No host-specific flag, alternate product markup or JS outside the existing Halcyon-owned transport.

## 4. UIKit picker on native iOS app shell
1. Extend the existing UIPickerView data-source presentation with derived Header and Item rows under the one existing ChoiceRequest. A Header is visually identifiable as a group title and is NEVER a valid OptionId or acceptance target.
2. If UIPickerView currently highlights a heading on Done, reject as no choice or keep the previous real selection without changing canonical state. Never map the header to the following real item or calculate an option index by subtracting an assumed single separator. Ensure a real chosen row maps to the exact original OptionId. Revisions/unmount/Cancel remain lease-qualified.
3. Set the initial actual highlighted row by the selected OptionId in the display mapping. Preserve existing Done/Cancel, modal lifecycle, main-thread ownership, keyboard/focus sovereignty and frame ownership. Do not create a second UIKit picker or app-specific delegate.
4. On devices without this host capability, preserve the same grouped Halcyon popup as the normal owner-correct fallback.

## 5. Verification and completion
1. Focused tests named hal401_ cover grouped two-section projection, 30+219 item view, edge headers and option positions, disabled/unselectable headers on Mouse/Touch/keyboard, scroll across boundary, selection once in either group, cancel, stale lease and exact ResolverInput.options content. Add 0, 1 and multiple boundaries and malformed input/refusal cases; existing flat path must remain equivalent.
2. WASM build and genuine browser SELECT fixture verify two optgroups, exact 249 option values with no extra option, original option order, first-Tap native SELECT delivery and pan never opening chooser. A real iPhone physical test is still needed to confirm UA's actual group display.
3. Native iOS UIKit target build and picker request/result mapping test cover header selection rejection, current chosen real row, Done and Cancel with no wrong neighboring option. No simulator compilation should be reported as interactive physical evidence.
4. Recheck only affected existing HAL-201/397/400 focused tests, normal rustc check and wasm32 target. No unnecessary unrelated audits.
5. Application catalog completeness and localization remain separate from framework acceptance. Use the 30+219 grouped fixture alongside independent 26-option and 5-option flat selectors. Select the first and last real options, preserve supplied labels and ordering, and retain the exact supplied OptionId through language changes and widget remounts. The framework defines no geographic catalog, network request or application-publication behavior.

## 6. Grouped popup bands and viewport fit (2026-11-22)

**Status: Adopted / Binding, custom grouped popup only.** JC approved clear
group hierarchy while preserving the existing selectable rows and host-native
pickers.

1. A heading occupies the existing 24-logical-pixel display row. Paint an
   opaque neutral band from the theme's list header or subheader background.
   Center muted text vertically at 82% of the option font size. Start the
   heading 4 logical pixels after the usual text padding and grouped option
   text 12 logical pixels after it. Option fills and highlight remain in
   their existing positions.
2. Each group after the first has a single physical-pixel line at the band's
   top inside the popup border. Use the theme's subtle border color, snap
   the line to the physical grid, and clip it to the existing rounded popup.
   The line consumes no additional row.
3. The caller supplies the exact heading. Fit it on one line in the
   scrollbar- and border-excluding text viewport. If it exceeds the
   available width, retain a fitting grapheme prefix with an ellipsis.
4. The grouped popup honors the requested display-row count up to the whole
   rows that fit beside the trigger in the root viewport. Eleven rows occupy
   264 logical pixels when space allows. The same admitted count and row
   origin govern the painted list, hit region, scroll range, thumb, and
   keyboard reveal. Neither placement nor animation may cover the trigger
   or overflow the viewport. Flat projection geometry and pixels, the
   Resolver and the HAL-400 native SELECT and UIKit presentation retain
   their existing behavior.

## 7. Grouped popup readability and persistent context (2026-11-22)

**Status: Adopted / Binding, custom grouped popup only.** JC's 17:16
scrolling observation supersedes §6's subdued heading treatment and
body-only popup height. The remaining geometry, ownership and identity rules
in §§2 and 6 continue to apply.

1. Keep each in-list heading at 24 logical pixels. Use a substantially
   differentiated neutral graphite band and readable neutral text at 95% of
   the option font size. Add a three-logical-pixel neutral left marker to
   each band and a two-physical-pixel snapped top rule to each later band.
   Clip both to the popup silhouette and keep the eight-pixel option indent,
   scrollbar exclusion and grapheme-safe title fitting.
2. Reserve a fixed 28-logical-pixel graphite footer beneath the scroll body.
   Show the supplied title on the left and the real item count on the right.
   Compute the count from adjacent boundaries or the end of the real option
   list. Keep the title and count legible and separated, with a neutral top
   border. Follow the group containing the first visible display row. The
   new group begins at its heading, including when that heading is the first
   visible row.
3. Reserve footer height before admitting whole body rows and choosing the
   placement. Eleven body rows and the footer need 292 logical pixels.
   Suppress the popup when neither side can fit one row and the footer. Use
   the same admitted rows and first row for painting, pointer hit testing,
   scrolling, thumb geometry and keyboard reveal. The track and thumb end
   within the body. The full overlay shields the footer from click-through.
4. The footer and headings receive no focus or choice press, carry no
   OptionId, and make no Resolver calls. Footer context follows scrolling
   without changing the selection or its owner. Flat projection appearance,
   geometry and lifetime, external current OptionId restoration, and native
   SELECT and UIKit presentation retain the existing contracts.
