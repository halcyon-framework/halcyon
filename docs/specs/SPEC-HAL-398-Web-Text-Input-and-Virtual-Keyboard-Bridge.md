# SPEC-HAL-398: Web Text Input and Virtual Keyboard Bridge
Status: Adopted / Active / Binding. Date: 2026-11-21.
Authority: JC's direct priority for fully usable Halcyon TextField, TextArea and text-enabled composite editors in WebAssembly on phone/tablet while preserving native shells and desktop keyboard/navigation.
Owner: existing Halcyon browser/window input transport, Focus/KeyboardAuthority, EventRouter and text-editing substrate.

## Scope and verification

- Objective: After one physical tap on an actual canvas TextField, the mobile soft keyboard appears and Unicode input edits the original Halcyon document; ComboBox retains Halcyon/Resolver choice authority while HAL-400 governs the device-native Touch picker projection; no desktop keyboard/focus regression.
- Consumer scenario: Text entry, choice selection and one-finger scrolling work together in the same Halcyon runtime, including a mobile form with an email field and selection-only controls.
- Governing: HAL-002/002A engine sovereignty, HAL-052 editing model, HAL-101 TextField, HAL-108 policy, HAL-201 ComboBox, HAL-397 touch activation, HAL-396 scroll, PM-014/023 composite ownership, current native IME action mapping.
- Source baseline at adoption: Halcyon src/platform/web/mod.rs implements BrowserAppLoop and PasteListener but no browser text-input owner; vendored winit-0.30.12 Web set_ime_allowed and set_ime_purpose are no-ops. TextField and TextArea already process typed Event::Text and Event::Ime under KeyboardAuthorityMode::Capture. Existing Windows/macOS/native iOS app shell paths must remain intact.
- Owned files: browser window/event loop transport, exact InputFusion/EventRouter keyboard ingress and existing text/composite action adapters, a single per-window text proxy, focused tests and representative existing example.
- Forbidden: independent DOM editor, HTML text field replacing canvas widget, OS-owned widget focus, hidden global proxy, untrusted edits without qualified session, duplicate key/IME delivery, user-click requirement repeated twice, background focus() to fake iOS keyboard, blanket preventDefault on unrelated events.
- Verification: exact typed bridge tests on genuine browser events and actual iPhone Safari/Brave/Android Chrome physical keyboard with Unicode/IME, backspace, selection, focus/blur, viewport; keyboard/mouse native preservation, wasm32 build. Unsupported platform behavior must be observed, not guessed.
- Effects: platform-local DOM transport only, no network/logging/persistence, no extra feature flag, no product-specific JS.
- Completeness: actual soft keyboard and text editing on device, not only fixture/compile success.

## Architectural boundaries

- Binding owners: this SPEC with HAL-002, HAL-052, HAL-101, HAL-108, HAL-201, HAL-397, PM-014/023.
- Widget policy: selection and edits remain with the same tree-owned editor and its TextEditPolicy.
- Interaction: host keyboard request only after a qualified, accepted trusted touch activation of actual editable control, or existing explicitly focused editing action.
- FORBID: no focus authority transfer to DOM, no duplicate text owner, no parallel editor, no implicit selection commit.
- Existing mechanism: web_sys, BrowserAppLoop, existing Canvas/paste transport, EventContext keyboard capture, Event::Text/Ime, original text substrate.
- Reinvention: do not implement another TextField or fork keyboard navigation.
- Demo: normal text input example uses the canonical widgets and original Escape behavior.

## 1. Host/engine ownership
1. Each live Halcyon browser canvas/window may own exactly one scoped ephemeral browser text-input transport through existing platform/web lifecycle. It may use a real focusable DOM input or textarea element located outside the rendered canvas's hit/paint surface. It is not a new visible product control or authoritative document/selection.
2. Engine Focus/KeyboardAuthority owns widget ID, window/domain, active editor session, capability and composition state. The DOM element is only a device transport for software keyboard and genuine browser text/IME events. It MUST NOT choose, change or retain widget focus/selection, data model, command intent, popup state or navigation.
3. The bridge is constructed and retired with the existing window/event-loop owner and a concrete mount/session lease; no singleton, permanent global listener, cross-window shortcut, widget reference, or alternate input channel. The canvas, routing and touch-action scope of HAL-396 stay unchanged.
4. Native desktop, macOS, iOS app shell and Android shell keep their existing input/IME route. Browser text transport is used only where the Web platform does not supply set_ime_allowed and native WindowEvent::Ime. Both converge on the SAME typed editing actions and original document authority.

## 2. Activation, focus and dismissal
1. A qualified accepted touch Tap on a TextField, TextArea or text-enabled ComboBox yields the original engine focus/KeyboardAuthority Capture decision. The platform adapter receives a tagged request for the exact window, editor mount and session; only then it may focus its input proxy as a transport.
2. On mobile WebKit, focus must execute synchronously within the original trusted user-activation event callback after engine admission. Do not place it behind a timer, promise, requestAnimationFrame, deferred frame or platform event replay. HAL-397 therefore MUST deliver the successful Tap/focus decision before the trusted touch terminal dispatch exits. If that is not possible with current Winit dispatch order, change the owner-correct synchronous event bridge, not the widget semantics or owner authority.
3. A pan or canceled Tap never opens the soft keyboard. Reject host focus when the target has unmounted, is disabled, not editable, in a different window/domain, or no longer owns KeyboardAuthority Capture.
4. On blur, focus transfer, disabled/read-only transition that exits edit mode, navigation, window teardown and user keyboard dismissal, retire only the matching keyboard lease and composition. Restore canvas focus only when the current engine focus owner requests it. Do not swallow physical Tab/Shift+Tab, Enter, Escape, shortcuts or application command navigation.
5. The host proxy may carry per-widget transport hints for the intended input class: an email keyboard for an email field, text/numeric/password hints where relevant, and multiline input for TextArea. These hints are presentation/capability only; they never override validation, policy, text contents or data type. No browser autofill/auto-submit determines application state.

## 3. Typed ingress and composition
1. The proxy must receive trusted beforeinput/input, compositionstart/update/end, selection, keydown/keyup and focus/blur events as the browser supplies them, all source-qualified by the current input lease. DOM changes are temporary observations only, never an additional canonical document.
2. Translate delivered input to existing Event::Text, Event::Ime(PreEdit/Commit/Cancel), deletion, selection and original text substrate operations in an ordered single-authority input envelope. Preserve Unicode scalar content, surrogate pairs, grapheme boundaries and actual browser IME preedit/commit behavior; no ASCII-only typing surrogate.
3. beforeinput can be non-cancelable or absent for autocorrect, autofill and IME. Therefore do not assume preventDefault implies complete ownership: observe input and reconcile each native edit exactly once through the original editor command path. No double insertion when beforeinput and input refer to the same transaction; no double composition commit when compositionend and input follow each other.
4. The DOM transport's temporary value/selection may mirror the minimum current editing window necessary to interface with IME/corrections, but MUST NOT become a second document truth, persist across editor sessions, expose secrets, or overwrite newer canonical editor state. Version/identity-check any reconciliation; invalid/stale edits are discarded and the host projection resynchronized from engine-authoritative facts.
5. When the bridge owns text input, ordinary canvas KeyboardInput/Ime from the same physical event must not also edit the document. Physical hardware-key navigation/shortcuts still route through existing engine authority exactly once, not into an isolated DOM-only handler. Clipboard follows current owner-qualified transport and privacy contracts.
6. Password input uses appropriate protected transport; do not log raw content or composition, expose a hidden DOM dump or send it over the network. At shutdown/unmount clear temporary buffers, remove listeners and retire proxy identity.

## 4. Viewport and accessibility boundaries
1. The host's visible viewport, including software keyboard occlusion and safe-area information, is an advisory input to the EXISTING window geometry/layout path. Do not independently shift widget coordinates or scroll document/body while ScrollArea is authoritative.
2. When keyboard changes the usable viewport, deliver one ordered size/occlusion update through the framework before focus reveal; preserve pending ScrollArea offsets, anchors and content extents as governed by existing layout/scroll authority. Keep current editor caret visible through existing reveal mechanism, never by direct DOM element scroll or canvas CSS offset.
3. The proxy must not introduce an independently visible form, overlay, focus outline, default native zoom or accidental hit region. Browser accessibility is a distinct host semantic projection under engine authority; do not mark the actual UI's meaningful control invisible merely because transport is hidden.
4. Browser support may differ. The normal path MUST work where the real HTML input focus and beforeinput/IME transport are available without requiring VirtualKeyboard API, experimental features, a hidden opt-in or global browser flags.

## 5. Acceptance
1. Native route tests: one tap on TextField requests matching keyboard lease; pan on same field does not; blur/unmount clears it; stale session cannot edit another field. TextArea newlines, backspace, Unicode/emoji, combining characters, IME preedit/commit/cancel, autocorrect replacement, selection, password, composition ordering, no duplicate key events.
2. Real browser test at 320/390 CSS logical widths on iPhone Safari and iPhone Brave: field activates at FIRST tap, keyboard visibly appears, user types/deletes into the same Halcyon TextField, Email draft remains across language/navigation; HAL-400 native picker handles genuine Touch selection-only ComboBox, original Halcyon popup remains for Mouse/Pen/physical keyboard; Escape and physical keyboard still behave correctly.
3. Browser keyboard occlusion/reveal checked with focus and scrolling, including device rotation. No JS/DOM hidden proxy visual artifacts. WASM release and ordinary native widget tests remain green.
