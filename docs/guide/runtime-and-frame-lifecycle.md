---
doc-id: halcyon-guide-runtime
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-001
  - SPEC-HAL-002
  - SPEC-HAL-018
  - SPEC-HAL-026
  - SPEC-HAL-030
  - SPEC-HAL-042
  - SPEC-HAL-043
  - SPEC-HAL-354
---

# Runtime and Frame Lifecycle

This document traces the live production frame path — `App::run`'s event-loop
closure in `src/core/app.rs` — at revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`. Two-axis claim tags per
SPEC-HAL-309 §7. The four dirty working-tree files
(`examples/showcase/tab_plot.rs`, `src/tests/market_detail.rs`,
`src/widgets/aligned_order_book.rs`, `src/widgets/mod.rs`) are not cited.

Governing authority split:

- Phase order is constitutional [authority: binding — SPEC-HAL-002 §5].
- Frame timing belongs to SPEC-HAL-041/042/043; SPEC-HAL-030 explicitly no
  longer owns "frame timing, pacing loops, redraw cadence, or animation
  clocks" and is narrowed to presentation policy
  [authority: binding — SPEC-HAL-030:20-24].
- Retained value updates and scene fragments are governed by SPEC-HAL-354
  [authority: binding — SPEC-HAL-354:12].
- Dirty-flag layout invalidation is governed by SPEC-HAL-026
  [authority: binding — SPEC-HAL-026:5 (`Status: Final`)], extended by 354
  which "forbids using layout dirtiness as a generic value-refresh signal"
  [authority: binding — SPEC-HAL-026:8].

## 1. Actors and Ownership

Constructed in `App::run` before the loop [implementation: implemented —
`src/core/app.rs:9115-9210`]:

| Owner | Decision it owns | Evidence |
|---|---|---|
| `EngineTicker` | Frame admission, reason, activity, throttling | `src/core/app.rs:739-752` (SPEC-HAL-042/043) |
| `ContinuousFramePacer` | The stable platform deadline for continuous work | `src/core/app.rs:692-718` |
| `AppRoot` | Screen/modal stacks, navigation queue, conditional rebuild | `src/core/app.rs:5611-5690`, `:10593`, `:10626` |
| `WidgetTree` | Node identity, dirty marks, structure generation, pending removals | `src/core/widget_tree.rs:205-251` |
| `EventRouter` + `InteractionDomains` | Focus, capture, routing state per domain (SPEC-HAL-151) | `src/core/app.rs:9199-9201` |
| `InputFusion` | Raw OS events → fused input | `src/core/app.rs:9208` |
| `build_scene_reusing` | Layout + retained scene composition, `visual_changed` verdict | `src/core/app.rs:1641-1664` |
| `RetainedSceneCache` | Fragment retention keyed by `TreeWidgetId` | `src/core/app.rs:2617-2622` |
| SceneGraph diff + `SceneCache` | Identity diff and cache dispatch at UI→render handoff | `src/core/app.rs:11200-11236` (SPEC-HAL-083) |
| `DrawPlanBuilder` | Build phase: scenes → DrawPlan | `src/core/app.rs:9135-9137`, `:11240-11242` (SPEC-HAL-084) |
| `WgpuRenderer` + `Compositor` | GPU execution and surface topology | `src/core/app.rs:9126-9133`, `src/compositor/mod.rs:16-25` |
| Presentation gate | Whether this frame is presented at all | `src/core/app.rs:11193-11198` |

## 2. The Screen Contract

Live trait: `src/core/app.rs:4785-4909` [implementation: implemented]. Hooks
and when they run:

- `build(&mut ScreenFrameContext)` — mounts the screen's root widget.
  Navigation is forbidden inside `build()` (doc comment at the trait).
  Called only when a rebuild is needed (§4).
- `update(&mut WidgetTree)` — every frame, "regardless of rebuild flag",
  for data-driven value updates on retained widgets; tree structure must not
  be modified here (trait doc). Called from `rebuild_active_view`
  [implementation: implemented — `src/core/app.rs:5680-5681`].
- `handle_input(FusedInputEvent, cx)` — modals top-first, then base screen,
  before widget-tree routing and global shortcuts; navigation allowed here
  (SPEC-HAL-018 Stage 7 doc on the trait).
- `flush_pending_actions(cx)` — once per frame after widget-tree routing,
  before scene build; poll widget sinks here for same-frame application
  (trait doc; see also `data-and-update-patterns.md`).
- `handle_app_event`, `handle_signal` — app-level events; signals arrive in
  the Signal Delivery Phase after input routing and before layout/scene/paint
  (trait doc; SPEC-HAL-161/162 machinery).
- Modal-only: `covers_point` (hit-test shielding, Stage 8),
  `dismiss_policy` (SPEC-HAL-127 Stage 4: OverlayRoot decides dismissal, the
  modal must not).
- `mark_rebuild` / `take_rebuild_flag` — Stage 12 incremental rebuild.
  **Default `take_rebuild_flag` returns `true` — a screen rebuilds every
  frame unless it overrides this pair** (trait doc). Consumer-relevant.
- `paint_child_window_scene` — bounded surface-scoped render path for
  runtime child OS windows (SPEC-HAL-151 keeps the retained tree
  primary-window-only; trait doc).

Divergence note: SPEC-HAL-018 (Draft) sketches
`Screen::build(&mut ScreenContext)` and an `AppRoot` implementing an `App`
trait with `build_scene()` left `unimplemented!()`
[authority: draft-intent — SPEC-HAL-018:102-109, :283-325]. The live trait
takes `ScreenFrameContext`, adds input/signal/update/modal hooks, and
`AppRoot` does not implement any `App` trait — `App` is a config facade
[implementation: implemented — `src/core/app.rs:9060-9074`]. Classified
draft-intent + implemented-divergent; recorded in §8.

## 3. Frame Admission and Cadence

- `FrameTime { now, dt, frame_index, reason }` is the single per-frame time
  snapshot [authority: binding — SPEC-HAL-042 per code doc |
  implementation: implemented — `src/core/app.rs:721-734`].
- `EngineTicker` carries pending `FrameReason`, engine activity, and last
  render time for throttling; non-Tick reasons mark the engine Active
  [implementation: implemented — `src/core/app.rs:739-780`].
- Idle throttle interval is 33 ms; continuous update/animation work targets
  ~120 Hz (`ACTIVE_FRAME_INTERVAL` = 8 333 µs, doc-tagged SPEC-HAL-354)
  [implementation: implemented — `src/core/app.rs:680-684`].
- `ContinuousFramePacer` owns one stable deadline that persists across
  event-loop callbacks. The doc comment states the failure it prevents:
  recomputing `now + interval` at callback entry and immediately requesting
  redraw from `AboutToWait` "bypasses `WaitUntil` and permits an update-only
  widget to spin the loop." `take_due` advances from observed time so a
  stalled loop never emits catch-up bursts
  [implementation: implemented — `src/core/app.rs:686-718`].
- Linux-specific redraw law: compositor-injected `RedrawRequested` events are
  gated; `request_redraw()` is forbidden inside `RedrawRequested` on Linux —
  all redraw scheduling goes through `AboutToWait`
  (INV-LINUX-REDRAW-1/2) [implementation: implemented —
  `src/core/app.rs:9368-9374`, `:9399-9415`, `:9801-9802`].
- Widget-side signals: `Widget::update(dt, cx)` +
  `Widget::needs_update()` (poll gate) and `Widget::needs_animation()`
  (unsettled visual effect only) [implementation: implemented —
  `src/core/widget/mod.rs:2105-2114`, `:2345`]. SPEC-HAL-354 assigns
  `needs_update` to attached receivers and reserves `needs_animation` for
  actually-unsettled effects [authority: binding — SPEC-HAL-354:195-200].
- The loop re-arms itself only while work exists:
  `any_widget_needs_update()` / active transitions set
  `FrameReason::Animation` and `animation_needed` before the throttle check
  [implementation: implemented — `src/core/app.rs:10705-10720`], and
  `animation_needed` is recomputed after scene build because routing or
  build may newly arm `needs_update`
  [implementation: implemented — `src/core/app.rs:11186-11191`].

Cadence invariants: one admitted engine frame performs at most one
update/layout/scene/presentation sequence per surface (INV-354-FRAME-1);
data ingress frequency cannot raise presentation above engine cadence
(INV-354-FRAME-2) [authority: binding — SPEC-HAL-354:232-235].

## 4. The Production Frame Sequence

Inside the primary window's `RedrawRequested` arm, after input routing
[implementation: implemented; each step cites its live line]:

1. Cursor defaulting honoring sticky-hidden state — `src/core/app.rs:10580-10590`.
2. Navigation queue processed — `app_root.process_nav_queue()`, `:10593`;
   exit request check `:10596-10599`.
3. SPEC-HAL-248 navigation frame projection pulled into `ScreenHost` —
   `:10601-10623`.
4. **Rebuild phase** — `app_root.rebuild_active_view(&mut widget_tree)`,
   `:10626`. Per base screen entry: teardown pending widgets, decide
   `needs_rebuild || screen.take_rebuild_flag()`; on rebuild, remove the old
   mounted node, call `Screen::build`, assert exactly one mounted root
   (`INV-REBUILD-ROOT`), and `mark_layout_dirty(root)`; otherwise skip.
   In both cases `screen.update(tree)` runs — the retained per-frame data
   path — then modals build conditionally bottom-to-top
   [`src/core/app.rs:5611-5690`].
5. Lifecycle cleanup — destroyed widgets drain to `EventRouter`
   (`on_widgets_destroyed`) with focus-chain recovery if the focused widget
   died; removed nodes drop signal subscriptions (SPEC-HAL-031); primary-
   window-only asserted (INV-151-PRIMARY-ONLY) — `:10628-10672`.
6. Focus scope sync against the modal stack — `:10674-10694`.
7. Activity arming — tooltip timer (SPEC-HAL-226A), `any_widget_needs_update`,
   active transitions → `notify_reason` + `animation_needed` — `:10696-10715`.
8. Throttle gate — `ticker.should_throttle(now)` may end the frame here —
   `:10717-10720`.
9. Runtime theme switch — `take_theme_changed()` swaps the local theme and
   `mark_subtree_dirty(active_root)` — `:10722-10726`.
10. SPEC-HAL-031 signal-driven dirty marks applied before layout —
    `:10728-10732`.
11. **Deferred Mutation Phase** (SPEC-HAL-190 §15.1) —
    `run_deferred_mutation_phase`: intents emitted during event routing are
    delivered to authority owners exactly once per frame if non-empty
    (INV-INTENT-PROCESS-1/2, best-effort on removed owners) —
    `:10734-10738`, contract at `:1288-1297`.
12. Pending composition actions (agent-driven structural tree mutation,
    SPEC-HAL-319 under SPEC-HAL-002 §3.8/§4.2: structural mutations happen
    here, never during event processing) — `:10740-10760`.
13. Layout-transition frame buffer swap
    (`begin_layout_transition_frame`, SPEC-HAL-078 Stage 0; prevents
    O(total_frames × widgets) candidate scans) — `:11052-11057`.
14. **Critical section + scene build** — `FrameCriticalSectionGuard`
    (SPEC-HAL-161: signal delivery forbidden during layout/scene/paint,
    same guard as `run_single_frame`) — `:11109-11111`; then
    `build_scene_reusing(...)` with the persistent `RetainedSceneCache` and
    recycled scene storage; `force_scene` only when runtime child windows
    exist — `:11113-11136`. Details in §5-§6.
15. SPEC-HAL-260 deferred focus + capture from the layout phase (widgets
    that mount children during layout, e.g. PropertyEditor's embedded
    TextField) — `:11139-11162`.
16. Tooltip manager updated from this frame's paint-emitted triggers —
    `:11164-11184`.
17. **Presentation gate** —
    `should_present = scene_outcome.visual_changed || !runtime_child_windows.is_empty()`;
    a presented frame must have produced a scene — `:11193-11198`.
18. If presenting: SPEC-HAL-083 identity diff
    (`extract_scene_keys` → `compute_diff_from_keys`) with debug identity-
    stability assertions, then `dispatch_cache_operations` (diff before
    render) — `:11200-11238`; SPEC-HAL-081B/163 Build phase over
    `compositor.snapshot_multi()` with per-surface scene ownership: the
    primary surface takes the scene by move — "SceneGraphs must NEVER be
    cloned across surfaces" (INV-SCENE-001) — child-window surfaces get
    scenes painted via `paint_child_window_scene_modal_first`, all others
    empty valid scenes — `:11240-11330`; then the live multi-surface
    tail, traced 2026-07-24: SPEC-HAL-295 deferred texture registrations
    drain (`renderer.drain_pending_image_registrations()`, `:11337`) →
    `draw_plan_builder.build_multi` over the surface scenes (`:11346`)
    with the primary scene recycled into storage (`:11358-11360`) →
    SPEC-HAL-081B compile phase (`renderer.compile_multi`, `:11366`) →
    execute phase (`renderer.render_multi`, `:11373`) → per-target
    platform submission (`submit_and_present`, INV-RG-013, `:11380`) →
    `advance_frame` (`:11385`) → `ticker.mark_rendered` (`:11406`) and
    the SPEC-HAL-317 `FrameRendered` agent event + frame counter
    (`:11409-11419`)
    [authority: binding — SPEC-HAL-084 per code docs `:1982-1983`, `:2056` |
    implementation: implemented].

`run_single_frame` (`:1989-2060`) is the canonical single-shot variant of
steps 14-18 (SPEC-HAL-042 Phase-2: layout → paint → render), used where no
ticker exists.

## 5. Update Phase and Invalidation Classes

The recursive update pass runs each widget's `update(dt, cx)` and then reads
the narrowest invalidation the widget reported:

```text
layout_invalidated          -> tree.mark_layout_dirty(node)
else subtree_paint          -> tree.mark_subtree_paint_dirty(node)
else paint || needs_anim    -> tree.mark_paint_dirty(node)
```

[implementation: implemented — `src/core/app.rs:1251-1282`, symbol
`update_widget_recursive`; linked-scroll projections sync first at `:1246`].

This is the live form of SPEC-HAL-354's invalidation domains: paint-only
must not claim geometry change; layout dirtiness is not a value-refresh
signal [authority: binding — SPEC-HAL-354:63-127, SPEC-HAL-026:8]. The
consumer contract is: update model → classify effect → mark paint-only or
layout+paint → return control; never call the renderer or request one frame
per data event [authority: binding — SPEC-HAL-354:283-296].

## 6. Retained Fast Path vs Full Rebuild — Not the Same Thing

Three distinct reuse levels, each with its own gate:

1. **Screen rebuild** is skipped per-screen unless flagged (§4 step 4).
   Skipping rebuild does not skip `screen.update`.
2. **Layout** is a no-op reusing the last completed geometry generation when
   the root is layout-clean and root constraints are unchanged
   [authority: binding — SPEC-HAL-354:202-206]. The retained paint path
   debug-asserts it only runs against valid completed geometry
   (`layout_epoch > 0 && !dirty_layout`)
   [implementation: implemented — `src/core/app.rs:2666-2671`]. Inside a
   layout pass, SPEC-HAL-026's five rules govern which nodes recompute
   [authority: binding — SPEC-HAL-026:192-218]. The in-scene-build pipeline
   ordering is engine law (SPEC-HAL-131A/C): pre-layout scroll sync →
   `layout_tree` → input projections → scroll anchors → focus reveal →
   list scroll sync → `paint_tree`
   [implementation: implemented — `src/core/app.rs:1665-1677`].
3. **Scene composition** replaces only dirty local fragments and recomposes
   only dirty ancestor paths. `RetainedSceneCache` holds
   `RetainedWidgetScene` entries keyed by `TreeWidgetId` plus a context key,
   structure generation, and transient drag/tooltip state
   [implementation: implemented — `src/core/app.rs:2617-2638`]. A clean
   node returns its cached fragment (`dirty_composition` gate,
   `:2673-2678`); recomposition is counted and
   `visual_changed = recomposed_fragments > 0 || transient_changed`
   [implementation: implemented — `src/core/app.rs:3139`]. An unchanged
   frame reuses the previous presented snapshot and is not presented
   (§4 step 17; INV-354-FRAME-3
   [authority: binding — SPEC-HAL-354:237-238]).

The fresh path exists too: `build_scene` wraps `build_scene_reusing` with a
throwaway cache and `force_scene = true` — "fresh build_scene must produce a
scene" [implementation: implemented — `src/core/app.rs:1580-1633`].

## 7. Presentation Semantics

- What is implemented: unchanged-frame suppression (§4 step 17), bounded
  update-only cadence with one stable non-Linux 120 Hz deadline, and the
  full retained pipeline — SPEC-HAL-354's own status section says so,
  records the 0.189.3-beta closure of the retained ListView selection
  dependency (pointer/keyboard/focus/hover changes invalidate the exact
  `ListViewBody` paint fragment; unchanged selection stays paint-clean),
  and names the remaining gap: "fragment-granular prepared draw-plan
  reuse is not claimed by this version"; a visually changed surface still
  traverses the full composed scene during draw-plan construction
  [authority: binding — SPEC-HAL-354:364-380 |
  implementation: partial — per that same section].
- Present-mode policy: SPEC-HAL-030 defines `PresentIntent`
  (LowLatency/BalancedVsync/StableVsync) and interactive present-mode
  overrides, and its own §10 declares explicit present-mode selection,
  the PresentIntent abstraction, swapchain mode switching, and
  platform-specific override logic **Not Implemented**
  [authority: binding-narrowed (`Status: Design — Partially Implemented`,
  SPEC-HAL-030:5) | implementation: absent — SPEC-HAL-030:224-230].
  Do not write consumer code assuming present-mode control exists.

## 8. Contradictions and Divergences (recorded, not harmonized)

1. SPEC-HAL-004 §3.4 narrates an every-frame loop
   (`app.handle_event` → `app.build_scene()` → present, SPEC-HAL-004:120-146)
   and §4.4 an unconditional render loop. Live code gates rebuild, layout,
   scene, and presentation independently (§4, §6). SPEC-HAL-004 is Draft
   (SPEC-HAL-004:5); SPEC-HAL-030:11-24 already records that timing moved to
   SPEC-HAL-041/042/043. Classification: draft-intent + superseded-in-part.
2. SPEC-HAL-018's Screen/App trait shapes vs the live `Screen` trait and
   `App` facade (§2). Classification: draft-intent + implemented-divergent.
3. SPEC-HAL-002 §5's six phases remain visible in the live sequence, with
   additional bounded stages (signal delivery, navigation projection,
   composition actions, deferred focus) inserted around them. SPEC-HAL-354
   authorizes phases proving no-op, not reordering
   (SPEC-HAL-354:192-193). Classification: binding + implemented
   (refinement, no observed order violation at the cited lines).
4. SPEC-HAL-030 PresentIntent machinery: binding-narrowed authority +
   absent implementation (its own §10). No consumer surface exists.

## 9. Executable Evidence (executed at this revision)

`src/tests/retained_value_updates.rs` (453 lines) holds the SPEC-HAL-354
proof set; exact test functions at this revision:

- `clean_root_and_paint_only_invalidation_reuse_completed_layout_geometry` (:140)
- `paint_invalidation_reaches_ancestor_composition_without_dirtying_sibling_layout` (:176)
- `update_context_distinguishes_paint_from_layout_invalidation` (:218)
- `retained_scene_repaints_only_the_dirty_widget_and_recomposes_its_ancestors` (:235)
- `unchanged_retained_frame_skips_scene_assembly_and_cache_is_bounded_by_tree_topology` (:301)
- `burst_data_publications_coalesce_into_one_paint_generation_without_layout` (:334)
- `update_only_polling_is_admitted_at_engine_cadence_under_event_loop_pressure` (:384)
- `retained_fragment_flattens_identically_to_fresh_scene_nodes` (:431)

Executed 2026-07-24 against a clean checkout of
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` (scratch `git worktree`,
`git status --porcelain` empty; rustc 1.97.1), command
`cargo test -p halcyon --lib retained_value_updates`: **8 passed,
0 failed** (1809 filtered out). Result axis for all eight pointers is
PASS at this revision (SPEC-HAL-309 §7: tests are executable evidence
only when run against the cited revision — this run satisfies that rule).
The full run record and the crate-wide test census live in
`audits/implementation-status.md` §3.
