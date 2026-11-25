# SPEC-HAL-397: Unified Touch Widget Interactions
Status: Adopted / Active / Binding. Date: 2026-11-21.
Authority: JC directly approved the unified input model and prioritized completion of native device interaction on WebAssembly and iOS/Android shells, without changing non-gesture mouse, pen or keyboard navigation.
Owners: existing InputFusion, EventRouter, GestureRegistry, pointer capture and widget semantic actions.

## Scope and verification

- Objective: One tap activates existing controls exactly once, touch-drag selects the right gesture, and native ScrollArea can pan from interactive surfaces without accidental activation.
- Consumer scenario: A Halcyon form on iPhone supports TextField editing, ComboBox selection, Checkbox and other control taps while one-finger scrolling continues to work. Existing desktop behavior is preserved.
- Authority: HAL-002/002A, HAL-101/108, HAL-112, HAL-128, locked HAL-131A, HAL-201, HAL-396, PM-002/014/018/023; this new SPEC supplements, not edits, locked contracts.
- Source baseline at adoption: Widget::accepts_touch_pointer_input defaults false; Button opts in; TextField, TextArea, ComboBox and Checkbox do not. TextField captures directly on PointerDown; ComboBox opens popup on PointerDown; Checkbox previews active press and captures on Down. Enabling touch alone would obstruct HAL-396 pan arbitration.
- Owned surface: existing qualified event ingestion, touch activation state, gesture registry, capture manager, widget implementations, focus/action owners and focused route tests.
- Forbidden: synthetic Mouse events, global mobile mode, second touch event router, consumer HTML widgets, cross-widget references, early value commits, broad source/type interpretation by widgets.
- Verification: real raw touch through InputFusion and EventRouter at 320/390 logical widths, native mouse/keyboard regressions, original device interaction and browser startup boundary. Preserve all unrelated source and behavior.
- Completion: functioning widgets, not merely compiled traits; HAL-398 separately supplies platform keyboard transport.

## Architectural boundaries

- Governing SPEC: this new touch profile plus HAL-002, 101, 112, 128, 201 and 396.
- Widget policy: present actions remain owned by their widget's existing state/action contract; new touch only provides a qualified initiation and cancellation path.
- Interaction: pending Tap, candidate Drag and ancestor Pan resolved through one existing GestureRegistry.
- FORBID: no OS authority over focus, capture, selection or commit; no double activation; no stale overlay or tree IDs; no separate input semantics for WASM.
- Reuse: HAL-396 qualified original press, first-contact pan, capture transfer, ScrollArea projection and current Focus/Action managers.
- Demo: current examples/ ownership and Escape rule remain; include a representative touch form demo.

## 1. Normal input preserved
1. Mouse, trackpad and Pen retain their original PointerDown/Move/Up, hover, wheel, drag, relative capture, DoubleClick and context-menu routing. Physical Tab, Shift+Tab, Enter, Escape, shortcuts and KeyboardAuthorityMode::Navigation/Capture retain exact behavior.
2. Device capability is per input source/contact, not a global is_mobile switch. PointerType, PointerId, window/domain, session, original physical touch press, overlay and mount are qualified in engine ingress. No widget performs PointerType inference or platform detection.
3. Touch does not synthesize Mouse events. Widgets receive a typed engine-admitted touch activation or gesture lifecycle event. InputFusion and EventRouter remain sole input authorities.
4. Ordinary Touch participation is explicit on supported interactive widgets in the normal build; ScrollArea remains gesture-only for Touch. Static labels and panels remain noninteractive. Enabled/read-only decisions remain current widget policy, not a new feature switch.

## 2. Pending touch and arbitration
1. Down identifies the live original hit path and creates exactly one pending interaction keyed by immutable qualified physical press, PointerId, widget mount, domain and overlay. Press visuals may be reversible; Down MUST NOT mutate checked/selected values, issue popup commits or start text editing.
2. A Tap is Up on the same live admitted target, with no committed other winner or terminal cancellation, and movement strictly below the existing HAL-396 4.0-logical-pixel recognition threshold. The engine delivers one typed action; no separate callbacks or synthetic Down/Up replay.
3. A qualifying Move uses the original HAL-396 gesture candidates, selected axis, first-contact restriction and priority: eligible non-Scroll gestures first, Scroll fallback. A Scroll winner atomically retires the pending widget interaction before notifying callbacks and applies finger motion once. No action on later Up, Cancel or late stale events.
4. A direct manipulation (slider/drag handle) wins only through an existing declared non-Scroll gesture candidate; not by hard capture on Down. Once it wins, ScrollArea cannot steal it. Recognition refusal leaves original press state and registry coherent and does not auto-retry/replay that batch.
5. Preserve HAL-112 Button unfinished-press transfer unchanged. For other controls use an additive typed owner-qualified pending-touch admission/cancel hook; DO NOT reinterpret the Button-specific has_unfinished_touch_press method as generic state. All fallible checks precede capture/registry publication. No whole-session cancellation for local transfer.
6. Click/tap and drag/pan are resolved only once; no duplicate activation after pointer terminal. Other contacts, Mouse capture and keyboard focus are unaffected except through existing explicitly admitted focus actions.

## 3. Widget behavior
- Button: retain HAL-112 exact press/tap/cancel, including exit-disarmed press; existing Mouse and keyboard paths unchanged.
- Checkbox, Switch, RadioButton: accepted Tap invokes the same existing Activate exactly once; pan/cancel restores reversible press paint without changing selected value. Keep their old Mouse Down/Up visuals and keyboard behaviors.
- TextField and TextArea: accepted Tap places caret via existing grapheme-safe hit-test, establishes existing keyboard Capture/focus and requests HAL-398 host keyboard activation in that trusted gesture. Touch Down alone does not start Mouse drag-selection or hard capture. Text selection by touch must be separately qualified; never corrupt current mouse selection.
- ComboBox: qualified Touch Tap on control/chevron uses the standard device-native picker under HAL-400 when the host supports it, while Mouse/Pen/physical keyboard retain the complete existing Halcyon popup. Selection or dismissal returns through the same Resolver-backed explicit OptionId path, with no value change on mere opening, no click-through and no activation after a Pan. Text-enabled ComboBox remains the existing tree-owned composite editor and HAL-398. HAL-400 explicitly supersedes the earlier touch-only prohibition on a native HTML select presentation.
- Slider/SpinBox: tapping a track or edit target and direct drag mutation are distinct admitted interactions; preserve current mouse and keyboard interactions. No value mutation before winning candidate or accepted Tap.
- Popup/menu/list choices: revalidate original mount/row/clip and current scroll geometry before commit. Never infer selection from a row touched during scrolling.
- Other existing interactive widgets adopt touch through their own previously approved semantic action and this same arbiter, not an unconditional blanket opt-in.

## 4. Lifecycle, tests and proof
1. Terminal Cancel, unmount, disable, domain/overlay retirement or viewport geometry change removes only the qualified pending owner and its reversible visual. No stale tap, repeated Up, click-through or release of another pointer.
2. Keep HAL-396 pure coordinate and capture handling: no second scroll offset, no widget traversal to find parent, no Touch entering Mouse scrollbar handlers, no full-session cancellation.
3. Real routed tests must cover Tap vs pan from enabled TextField, TextArea, ComboBox body/chevron/row, Checkbox/Radio/Switch and Slider/SpinBox; focus/commit once, proper cancellation, owner/capture and other contact isolation. Repeat existing mouse drag-selection, popup, wheel and keyboard/navigation tests only at changed boundaries.
4. On actual iPhone browser: tap TextField/ComboBox/checkbox once; pan beginning on each control scrolls page without action; HAL-400 proves native picker selection and cancellation on touch ComboBox; HAL-398 proves virtual keyboard. The original Halcyon popup remains valid for Mouse/Pen and keyboard. On desktop all current mouse/keyboard behavior remains unchanged.
