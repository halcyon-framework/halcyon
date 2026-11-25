---
doc-id: halcyon-guide-input
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-009
  - SPEC-HAL-020
  - SPEC-HAL-033
  - SPEC-HAL-176
---

# Input, Events, Focus, and Actions

Revision `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; two-axis tags per
SPEC-HAL-309 §7. Constitutional ground rules: no direct UI mutation from
callbacks, no interpretive intent resolution, deferred mutation only
[authority: binding — SPEC-HAL-002 §3.7, §3.11, §4.2].

Sections 1–8 retain that historical revision. Section 9 is a separately pinned
native-contact continuation; it does not reverify the older sections.

## 1. The Input Path, End to End

```text
OS events (winit)                    platform transport only
  -> InputFusion                     raw -> fused (SPEC-HAL-033)
  -> Screen::handle_input            modals top-first, then base screen
  -> widget-tree routing             EventRouter, phases, capture
  -> global shortcuts / actions      binding registry -> ActionId
  -> Screen::flush_pending_actions   same-frame sink polling
  -> Deferred Mutation Phase         intents to authority owners (SPEC-HAL-190)
```

Order evidence: trait doc on `Screen::handle_input` ("Called on modals
(top-first) and base screen before global shortcuts",
`src/core/app.rs:4785-4909`), `flush_pending_actions` ("after widget tree
routing, before build_scene", same trait), deferred mutation phase after
routing and before layout (`src/core/app.rs:1288-1300`, `:10734-10738`).

## 2. Input Fusion (SPEC-HAL-033)

`InputFusion` converts raw platform events into fused high-level events:
multi-click detection, drag recognition, scroll normalization, shortcut
matching [authority: draft-intent — SPEC-HAL-033:5 (`Status: Draft`) |
implementation: implemented — `src/core/input_fusion.rs:1-4`].
Live constants: double-click window 200 ms (SPEC-HAL-036 Phase 5), click
distance threshold 4 logical px
[implementation: implemented — `src/core/input_fusion.rs:21-22`, `:44-45`].
The fused vocabulary (`FusedInputEvent`, `KeyCode`, `Modifiers`,
`PointerButton`, `WheelDelta`, `ScrollSource`, `ShortcutId`,
`ShortcutScope`, `WindowId`, …) lives in `src/core/event/mod.rs`
(imports at `src/core/input_fusion.rs:7-11`). The production loop owns one
`InputFusion` instance [implementation: implemented —
`src/core/app.rs:9208`].

## 3. Widget Event Surface

`Widget::handle_event(cx, event, phase) -> bool` plus focus/capture/hover/
gesture/drag hooks [implementation: implemented —
`src/core/widget/mod.rs:2121`, `:2353-2372` (focus/capture enter/leave),
`:2515-2526` (hover), `:2550-2593` (gesture surface/commit/cancel),
`:2739-2765` (drag payload/accept/drop)]. Event phases and
tunnel/target/bubble routing are drafted in SPEC-HAL-009
[authority: draft-intent — SPEC-HAL-009:5 (`Status: Draft`)]. The live
`EventRouter` (`src/core/event_router.rs:70-118` fields; 76 members
inventoried 2026-07-24, `inspect EventRouter facets members` complete)
implements the pipeline concretely: phase-asserted entry
(`route_pointer_event` `:399`, `route_key_event` `:676`,
`route_input_event` `:3685`), capture-phase-then-target-and-bubble
dispatch (`dispatch_capture_phase_only` `:928`,
`dispatch_target_and_bubble` `:978`, `dispatch_to_widget` `:1089`),
gesture arbitration (`resolve_gesture_commits_for_pointer` `:1384`),
pointer-session cancellation (`:1624-1810`), focus/scope/escape/
directional navigation (`:2102-2986`), hover (`update_hover` `:3281`),
and drop-target resolution (`:3652`) [implementation: implemented].

Hit-testing is widget-tree-owned (`hit_test_grid`, `hit_test_nodes`,
`overlay_hit_regions` — `src/core/widget_tree.rs:213-218`); the compositor
is forbidden from hit-testing (INV-COMP-N11,
`src/compositor/mod.rs:25`).

## 4. Focus and Capture

- Per-domain focus state: `InteractionDomains` (SPEC-HAL-151) holds
  `focus_manager`, `pointer_capture_manager`, `keyboard_mode`, drag/drop
  session [implementation: implemented — `src/core/app.rs:11060-11067`].
- Focus recovery on rebuild: if the focused widget is destroyed, the router
  rebuilds the focus chain and re-focuses by chain index
  [implementation: implemented — `src/core/app.rs:10642-10666`].
- Focus scopes sync with the modal stack every frame; focus is forced back
  into the active scope [implementation: implemented —
  `src/core/app.rs:10682-10694`]; scope filtering is tree-assisted
  (`filter_focus_scopes`, `src/core/widget_tree.rs:933`;
  `is_focus_scope`/`defines_focus_scope`,
  `src/core/widget/mod.rs:2439`, `:2502`).
- Deferred focus from layout-mounted children (SPEC-HAL-260): applied
  after scene build, entering Capture only if the widget is
  `capture_eligible()` [implementation: implemented —
  `src/core/app.rs:11139-11162`].
- Modal shielding: `Screen::covers_point` decides pointer blocking;
  dismissal belongs to OverlayRoot via `Screen::dismiss_policy`
  (SPEC-HAL-127 Stage 4) — the modal must not dismiss itself
  [implementation: implemented — trait docs, `src/core/app.rs:4785-4909`].

## 5. Keyboard Authority Modes (SPEC-HAL-176)

Two live modes [authority: SPEC-HAL-176:3 (`Design / Binding`) |
implementation: implemented — `src/core/event/mod.rs:62-69`]:

- `Navigation` — "Keyboard events route through binding registry.
  hjkl/arrows for spatial nav, Tab for structural nav, Enter to
  activate/enter."
- `Capture` — "Focused widget owns all keyboard input. Only
  release_capture (Escape) is handled by the engine."

The mode is per-domain state feeding scene build and tooltip suppression
(`src/core/app.rs:11063`, `:11068-11073`).

## 6. The Action System (SPEC-HAL-020/A/B/C/D)

Authority: the 020 family is `Design / Binding` — core model (020A),
resolution/dispatch (020B), binding/input mapping (020C), execution/scope
lifecycle (020D) [authority: binding-design — SPEC-HAL-020:3,
SPEC-HAL-020A:3, SPEC-HAL-020B:3, SPEC-HAL-020C:3, SPEC-HAL-020D:3].

Live machinery in `src/runtime/` (SPEC-HAL-020 module,
`src/runtime/mod.rs:1-11`):

- `ActionRegistry { actions, shortcut_map, list, binding_registry }` with
  `add`, `find_action_by_shortcut`, `execute(id, &mut AppContext)`,
  `try_execute_shortcut`, scope filtering, and basic/fuzzy search (command
  palette support) [implementation: implemented —
  `src/runtime/actions.rs:333-620`].
- `BindingRegistry` — four binding layers indexed by `BindingLayer`, a
  resolved `InputPattern -> ResolvedBinding` map with dirty-flagged
  `resolve()`, `lookup(pattern) -> Option<ActionId>`, tag-scoped removal
  [implementation: implemented — `src/runtime/binding_registry.rs:123-221`].
- `register_builtin_actions` installs the built-in set
  [implementation: implemented — `src/runtime/mod.rs:7-11`].

Consumer rule (project law, mirrored here): never hand-roll keyboard
handling in widgets/screens for behavior the action system covers — define
an `Action`, bind an `InputPattern`, let resolution dispatch it. In
`Navigation` mode the binding registry IS the keyboard router (§5).

## 7. Intents — Deferred by Construction

Widget interactions that mutate an authority owner's state are expressed as
intents, buffered on the tree (`intent_buffers`,
`src/core/widget_tree.rs:245`, `:879-888`) and delivered once per frame in
the Deferred Mutation Phase (SPEC-HAL-190 §15.1: INV-INTENT-PROCESS-1/2,
best-effort on removed owners)
[implementation: implemented — `src/core/app.rs:1288-1300`, `:10738`].
Tabular widgets add their own typed intent queue on the substrate
(`pending_intents: Vec<TabularIntent>`, `on_intent` observer —
`src/widgets/tabular_view/mod.rs:436-439`; SPEC-HAL-177
`Implemented / Closed`, SPEC-HAL-177:3). Same-frame *action sink* polling
belongs in `Screen::flush_pending_actions`, not in per-event
`handle_input` — per-event code cannot see same-frame sink writes from
dispatch (trait doc, `src/core/app.rs:4785-4909`).

## 8. Recorded Divergences

- SPEC-HAL-009 remains Draft. The header-attribution finding was resolved
  on 2026-11-23 under HAL-309's maintainer-authorship rule; that editorial
  correction does not adopt the draft's behavioral requirements.
- SPEC-HAL-033 is Draft while the fusion engine is live (§2):
  draft-intent + implemented.
- Gesture-driven redraw escalation is real (`Stage 42I` per
  SPEC-HAL-030:18) — cadence effects of input are covered in
  `runtime-and-frame-lifecycle.md` §3.

## 9. Native Contacts and Button Activation

Verified 2026-10-21 against code revision
`15040b309cbcb425f970e3e440e88c08362bbdfd` (`0.221.0-beta`).
The scoped contracts are binding HAL-112 §3 and HAL-128 §2.1; Draft HAL-033
and HAL-079 are not promoted by this implementation.

The platform adapter transports all four native contact phases, preserving
the native ID and normalizing coordinates once. InputFusion assigns checked
nonzero logical pointer IDs per contact lifetime and retains the original
qualified press and Touch type. Native ID zero cannot alias Mouse; focus loss
cancels contacts before its focus terminal
[implementation: implemented — `src/platform/window.rs:422`,
`src/core/input_fusion.rs:995`, `src/core/event/mod.rs:2891`].

Touch Down/Move/Up use the existing four-phase pointer route. Capture, release
and commitment use the actual pointer ID; the old single-pointer capture
mirror remains Mouse-only. `Widget::accepts_touch_pointer_input` is a static
opt-in, default false and true for Button. An inside press arms Button, first
exit disarms it, and only the initiating contact's inside release can activate
it. Touch does not implicitly move keyboard focus or manufacture hover
[implementation: implemented — `src/core/event_router.rs:1568`,
`src/core/event_router.rs:5082`, `src/core/widget/mod.rs:3117`,
`src/widgets/button.rs:934`, `src/widgets/button.rs:1126`].

Contact cancellation bypasses Screen/modal consumption and reuses tree-aware
terminal teardown. Modal dismissal shields only the initiating contact's
remaining sequence. iOS suspension retires normalized, queued and deferred
touch input before render-target teardown
[implementation: implemented — `src/core/app.rs:2071`,
`src/core/app.rs:2163`, `src/core/app.rs:8158`, `src/core/app.rs:14462`].

The six public `touch_contact_` regressions in
`tests/gesture_cancellation_stage4.rs`, seven platform/Fusion inline tests and
three App boundary tests passed. These are synthetic path and lifecycle
proofs. A separate iOS consumer also built, installed and launched with a
Complete Metal frame receipt. The maintainer's subsequent manual Simulator
check recorded twelve Button Down events, twelve Up events and twelve
activation callbacks. The final activation at 09:43:15.654614 UTC was followed
by Complete realization of the updated counter, submitted and presented.
Long-press selection, double-tap recognition, pan/scroll and pinch recognition
remain absent from this scoped implementation; HAL-128's Touch+Select
restriction is unchanged.
