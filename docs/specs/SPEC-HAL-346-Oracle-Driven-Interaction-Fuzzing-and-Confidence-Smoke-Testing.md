# SPEC-HAL-346: Oracle-Driven Interaction Fuzzing and Confidence Smoke Testing

**Status:** Draft
**Layer:** Verification / Interaction Runtime / Developer Tooling
**Audience:** Engine, Widgets, Interaction, Layout, CI, Diagnostics
**Owner:** Engine integrity

**Depends on:**

* SPEC-HAL-002 (Constitution and Architectural Invariants)
* SPEC-HAL-009 (Event Routing and Hit-Testing)
* SPEC-HAL-024 (Coordinate Model)
* SPEC-HAL-033 (Input Fusion Layer)
* SPEC-HAL-036 (Input Coalescing)
* SPEC-HAL-079 (Unified Event Flow v3)
* SPEC-HAL-086 (Scroll and Viewport System)
* SPEC-HAL-128 (Pointer Capture and Gesture Arbitration)
* SPEC-HAL-129 (No-Stub Test Policy)
* SPEC-HAL-131 (Virtualization Projection and Scroll Contract)
* SPEC-HAL-141 (Rect Mutation Authority)
* SPEC-HAL-146 (Temporal Ownership of Geometry)
* SPEC-HAL-148 (Stable Widget Identity)
* SPEC-HAL-158 (Layout Authority and Invalidation Law)
* SPEC-HAL-164 (TabularSubstrate Root Abstraction and Invariants)
* SPEC-HAL-168 (Horizontal Geometry Resolution)
* SPEC-HAL-169 (Column Resize Interaction Contract)
* SPEC-HAL-317 (Agent Integration Architecture)
* SPEC-HAL-317A (Semantic Agent Event Model)

---

## 1. Purpose

This SPEC defines Halcyon's oracle-driven interaction fuzzing and
confidence smoke-testing subsystem.

The subsystem generates deterministic, structurally valid interaction
transactions, drives them through Halcyon's real internal input path,
observes semantic state through explicit probes, and verifies declared
invariants after every transaction.

Default user-like generated interactions MUST model the path to an
intended action, not only the final coordinate or final widget command.
Incidental hover, focus, hit-test, scroll, overlay, and layout
transitions that occur while moving toward an intended action are part
of the interaction under test.

This SPEC defines two complementary execution families:

1. Contract/user-like fuzzing: intent-led interaction generation with
   semantic oracles and bounded confidence packets.
2. Chaos/bombardment fuzzing: primitive-biased random pressure against a
   declared app or root surface, guarded by hard-failure oracles,
   deterministic recording, shrinking, and replay.

Chaos/bombardment fuzzing is a failure-discovery mode. A passing chaos
run may report executed budget, corpus deltas, and coverage deltas, but
it MUST NOT emit a full interaction-confidence claim.

Its purpose is twofold:

1. Find interaction bugs that example tests and manual testing miss.
2. Produce bounded confidence statements that important interaction
   contracts continue to work.

The second point is binding. This subsystem is not only a crash finder
or failure reporter. It is a primary source of confidence that Halcyon
still works across its interaction frontier.

---

## 2. Motivation

Halcyon's failure-prone zones are interaction state machines under
pressure:

* pointer capture lifecycle
* drag threshold activation
* resize anchor direction
* coordinate-space conversion under scroll
* header/body/footer geometry synchronization
* layout invalidation after mutation
* virtualization identity stability
* focus and keyboard authority transfer
* overlay and modal input boundaries
* stale geometry after first interaction
* repeated interactions after state normalization

Manual testing finds these problems inconsistently. A developer may
notice a defect once, fail to record a reproduction, and lose it for
days. Example tests prove named cases, but they do not explore the
interaction state space.

Many failures occur before the intended action, while the pointer or
keyboard focus is merely traveling through the interface. Crossing a
separator, hover zone, overlay boundary, stale rect, scrollbar, row, or
focusable child is therefore not noise. It is required signal.

This SPEC creates a deterministic system that can:

* exercise meaningful interaction sequences repeatedly
* shrink failures to minimal replay artifacts
* report exact violated contracts
* promote reproductions into regression tests
* state what has been exercised and what remains outside the tested
  frontier

---

## 3. Core Claim

The fuzzer MUST NOT claim global correctness.

It MAY claim bounded confidence:

> No invariant violations were found across the executed interaction
> contract frontier, under the declared fixture set, generator set,
> seed set, transaction budget, and semantic coverage frontier.

This is the only valid positive claim.

---

## 4. Foundational Principles

### CP-346-1: Real Internal Input, Not OS Automation

The fuzzer MUST NOT move the platform cursor.

The fuzzer MUST inject synthetic input at Halcyon's internal input
boundary:

```text
RawInputEvent -> InputFusion -> FusedInputEvent -> EventRouter -> Widget
```

The path after injection MUST be the same path used by platform input.

### CP-346-2: Read Deep, Write Shallow

The fuzzer MAY inspect deep semantic state through explicit probes.

The fuzzer MUST mutate UI state only through input transactions or
authorized test harness setup.

### CP-346-3: Oracle Before Generator

No generator is valid until at least one oracle exists for the
interaction domain it exercises.

Random action generation is valid only in declared chaos/bombardment
mode and only when the hard-failure oracle set is enabled.

Chaos/bombardment mode is not user-like contract fuzzing. It MUST
preserve the internal input boundary, deterministic replay recording,
artifact generation, and Constitution ownership boundaries, but it does
not require a semantic intended target for each command.

Random action generation without either a semantic contract oracle or a
hard-failure oracle set is forbidden.

### CP-346-4: Contracts, Not Screenshots

The primary oracle is semantic state, not pixels.

Screenshots MAY be attached to failure packets, but they MUST NOT be
the only proof of failure for an interaction contract.

### CP-346-5: Shrink First, Report Second

A failure MUST be minimized before being reported as actionable, unless
the failure is non-shrinkable because startup, fixture construction, or
the first transaction fails.

### CP-346-6: Confidence Is Coverage-Bound

The subsystem MUST report what it covered, not merely that it passed.

A passing run without coverage metadata is not a confidence artifact.

### CP-346-7: Deterministic Replay Is Mandatory

Every failure must be replayable from a stored artifact without timing
guesswork, network dependency, or live manual input.

### CP-346-8: No Hidden Oracle

All assertions must be named, declared, and tied to a contract,
invariant, or metamorphic property.

An implicit "looks wrong" judgment is not an oracle.

### CP-346-9: Incidental Interaction Is Real Input

The fuzzer MUST NOT collapse a user-like interaction into only its final
coordinate, final focus target, or final semantic command.

Intermediate movement, hover, focus traversal, scroll, capture,
hit-test, layout, and overlay transitions are runtime behavior and MUST
be routed, probed, oracle-checked, and recorded unless the trace
explicitly declares a primitive replay or injection-boundary test mode.

### CP-346-10: Constitution First

The fuzzer is verification infrastructure, not an alternate runtime.

It MUST preserve SPEC-HAL-002 authority boundaries. No generator,
probe, oracle, shrinker, or reporter may take ownership of widget,
layout, render, timing, identity, scroll, focus, or capture authority.

The fuzzer may request behavior only through declared input
transactions or authorized fixture setup. It may observe through probes.
It may not repair, recompute, normalize, or backfill runtime state.

### CP-346-11: No Fixture Shortcut After Start

Authorized fixture setup ends before the first interaction transaction.
After the first transaction begins, the fixture may change only through
the same runtime paths available to the interaction being tested.

Resetting a fixture is a new run, not a transaction inside the current
trace.

### CP-346-12: Target Resolution Is Observation, Not Mutation

Resolving an intent target may read declared probes and current runtime
geometry. It MUST NOT force layout, mutate caches, rebuild widgets,
scroll containers, or synthesize missing geometry.

If a target cannot be resolved from an already-settled runtime state,
the intent expansion must fail with a skipped-generator reason or a
fixture/setup failure. It must not self-heal the runtime to obtain a
coordinate.

### CP-346-13: Diagnostics Are Evidence, Not Authority

Visual diagnostics, command timelines, overlays, and frame captures are
derived evidence. They MUST be generated from recorded traces, runtime
probes, scene output, or rendered frames already produced by Halcyon.

Diagnostics MUST NOT become an oracle, mutate runtime state, move the OS
cursor, call browser automation, infer semantic truth from pixels, or
replace any required semantic or hard-failure oracle.

### CP-346-14: Negative Proof Is Required

Every oracle, hard-failure detector, replay validator, shrinker
validator, reporter validator, and Constitution boundary check required
by this SPEC MUST have a deterministic negative validation case proving
that it fails closed.

Negative validation MUST use declared test surfaces, synthetic oracle
inputs, or invalid replay/report artifacts. It MUST NOT mutate live
runtime state outside fixture setup, bypass the input boundary, or move
authority from Halcyon runtime owners into the fuzzer.

---

## 5. Non-Goals

This SPEC does not define:

* platform cursor automation
* browser automation
* visual snapshot testing as the primary oracle
* probabilistic UI acceptance criteria
* AI judgment of rendered UI correctness
* production telemetry
* public Agent API transport changes
* a replacement for unit tests or regression tests

The subsystem complements unit, integration, and regression tests. It
does not replace them.

---

## 6. Terminology

**Interaction transaction**  
One complete externally supplied input unit plus the required engine
settling phases:

```text
input command(s)
-> input fusion
-> event routing
-> widget mutation
-> deferred mutations
-> rebuild/layout/reconcile
-> paint/scene update as required
-> invariant checks
```

**Command**  
A deterministic input instruction emitted by the fuzzer driver, such as
pointer move, pointer down, pointer up, wheel, key down, or text input.

**Trace**  
An ordered list of commands plus frame/reconcile boundaries.

**Fixture**  
A deterministic UI setup with declared widget tree, data, theme,
dimensions, clock, scale factor, and optional initial state.

**Chaos surface**
A deterministic app or root UI surface declared for bombardment. It may
cover a broader application entry state than a semantic fixture, but it
MUST still declare surface identity, build identity, window size, scale
factor, theme, clock, seed, enabled hard-failure oracles, and artifact
directory.

**Chaos command stream**
A seeded primitive command sequence generated without a required
semantic intended target. It records pointer movement samples,
pointer-button state, wheel bursts, key state, text input, window/scale
changes, frame boundaries, reconcile boundaries, generated coordinates,
and generator version.

**Interaction fuzz profile**
A complete run configuration declaring mode, fixtures or chaos surfaces,
oracles, seeds, budgets, coverage window, timeout/no-progress rules,
artifact directory, diagnostics policy, and Constitution boundary
profile.

**Visual trace**
A deterministic diagnostic artifact derived from a recorded trace and
runtime observations. It may show pointer path, command timeline,
capture/focus/hit-test targets, dirty/settle markers, frame snapshots,
and failure context. It is evidence for human inspection, not an oracle.

**Negative validation surface**
A deterministic test-only surface or synthetic artifact used to prove an
oracle, detector, reporter, replay path, or Constitution boundary check
fails closed when given a known-bad condition.

**Probe**  
A read-only semantic inspection surface for a widget, substrate, or
engine layer.

**Oracle**  
A deterministic checker that compares pre-state, command, post-state,
and contract metadata.

**Hard-failure oracle**
A deterministic checker for runtime failure classes that do not require
a semantic intended target: panic, crash, timeout, non-finite sink
geometry, capture leak, dirty-settle violation, router contradiction,
authority violation, replay nondeterminism, or unsupported-command
handling failure.

**Invariant**  
A state property that must hold after every relevant transaction.

**Metamorphic property**  
A property over related traces, such as "drag + inverse drag restores
the original geometry within declared rounding rules."

**Coverage key**  
A semantic abstraction of state or transition used to measure frontier
exploration.

**Failure packet**  
The minimized, replayable artifact describing one violated oracle.

**Confidence packet**  
The positive artifact emitted by a passing run, describing fixtures,
contracts, coverage, seeds, budgets, and remaining boundaries.

---

## 7. Architectural Overview

The subsystem consists of eleven parts:

1. Fixture and chaos surface registry
2. Interaction command driver
3. Transaction and chaos runner
4. Semantic probe registry
5. Oracle registry
6. Generator registry
7. Chaos stream generator
8. Coverage tracker
9. Shrinker
10. Reporter
11. Corpus manager

Contract/user-like pipeline:

```text
Fixture
  -> Generator
  -> Command Trace
  -> Interaction Driver
  -> Halcyon Runtime
  -> Semantic Probes
  -> Oracles
  -> Coverage Tracker
  -> Failure Packet or Confidence Packet
```

Chaos/bombardment pipeline:

```text
Chaos Surface
  -> Chaos Stream Generator
  -> Primitive Command Trace
  -> Interaction Driver
  -> Halcyon Runtime
  -> Hard-Failure Oracles
  -> Recorder
  -> Shrinker
  -> Failure Packet or Corpus Entry
```

---

## 8. Relationship to Agent API

SPEC-HAL-317 defines external agent observation and action.

This subsystem reuses the design principle, not the transport.

Agent API:

* external controller
* event channel
* action channel
* useful for human-facing tooling and remote automation

Interaction fuzzer:

* in-process
* deterministic
* no WebSocket dependency
* no network timing
* direct fixture and probe access
* direct replay and shrink support

The two systems MAY share command, snapshot, and report schemas later.
The first implementation MUST be an in-process developer/testing
subsystem.

---

## 9. Input Boundary

### 9.1 Required Injection Layer

The fuzzer MUST inject commands before `InputFusion` unless a test mode
explicitly targets `EventRouter` with pre-built `FusedInputEvent`s.

Default path:

```text
InteractionCommand
  -> RawInputEvent
  -> InputFusion
  -> FusedInputEvent
  -> EventRouter
```

### 9.2 Router-Only Mode

Router-only mode is allowed only for tests whose stated target is
routing, capture, or widget handling after fusion.

Router-only mode MUST be labeled in the report.

### 9.3 Forbidden Injection

The fuzzer MUST NOT:

* call widget event handlers directly
* mutate widget fields directly
* mutate column widths directly when testing resize
* downcast widgets for write access after fixture setup
* use probe code to repair or normalize runtime state
* force layout, hit-test rebuild, or geometry recomputation from target
  resolution
* bypass `InputFusion` in default interaction mode
* bypass `EventRouter`
* synthesize "already committed" widget semantic events as input

### 9.4 Constitutional Boundary

The permitted ownership model is:

```text
Generator owns intent selection.
Intent expander owns primitive command emission.
InputFusion owns raw-to-fused input semantics.
EventRouter owns routing, capture, hover, focus, and event phases.
Widgets/substrates own their interaction state.
Layout owns resolved geometry.
Probes own read-only snapshots.
Oracles own pass/fail judgment.
Reporter owns artifacts.
```

No layer may write across this boundary to make a test pass or to make a
target resolvable.

The runner may advance normal runtime phases, but it must do so through
the same frame/reconcile machinery used by the application. It must not
call private layout or widget mutation helpers as a substitute for an
input transaction.

Any test mode that intentionally narrows the boundary, such as
router-only mode, must label the omitted layers in the report and must
not produce a full interaction-confidence claim.

---

## 10. Command Model

The command model MUST be explicit and replayable.

The command model has two layers:

1. intent commands
2. primitive commands

Intent commands describe user-level navigation and action. Primitive
commands describe the exact injected input stream.

Generators MUST emit intent commands by default. The runner MUST
expand intent commands into primitive commands before injection. Replay
artifacts MUST record both layers when an intent command was used.

Primitive-only traces are valid for declared chaos/bombardment mode,
exact replay, shrinker output, and tests that explicitly target the
injection boundary. Primitive-only generation MUST NOT be the default
for user-like interaction fuzzing.

Generated user-like interaction fuzzing MUST model sampled navigation
through intermediate states. Resolving a target and injecting only the
final endpoint is endpoint teleportation. Endpoint teleportation is
invalid for generated user-like interaction even when the final state
oracle passes. It is allowed only for exact primitive replay, shrinker
minimization, or tests explicitly targeting direct coordinate injection.

### 10.1 Primitive Commands

Required primitive command families:

```text
PointerMove { x, y, device, modifiers }
PointerDown { x, y, button, device, modifiers }
PointerUp { x, y, button, device, modifiers }
Wheel { x, y, delta_x, delta_y, source, modifiers }
KeyDown { key, modifiers, repeat }
KeyUp { key, modifiers }
TextInput { text }
Frame { count }
Reconcile
SetWindowSize { width, height }
SetScaleFactor { scale_factor }
```

Coordinates are logical window coordinates unless explicitly marked
otherwise.

### 10.2 Intent Commands

Required intent command families:

```text
MovePointerTo { target, path_policy, settle_policy }
MovePointerBy { dx, dy, path_policy, settle_policy }
HoverTarget { target, path_policy, hover_frames }
ClickTarget { target, button, path_policy }
DoubleClickTarget { target, button, path_policy }
DragTarget { from, to, button, path_policy, drag_policy }
DragBy { from, dx, dy, button, path_policy, drag_policy }
WheelAtTarget { target, delta_x, delta_y, source, path_policy }
FocusNext { count }
FocusPrevious { count }
FocusTarget { target, navigation_policy }
ArrowNavigate { direction, count, modifiers }
PageNavigate { direction, count, modifiers }
HomeEndNavigate { edge, modifiers }
ActivateFocused
TypeTextIntent { text, text_policy }
KeyChord { keys, modifiers }
```

Intent targets MUST be semantic targets or probe-derived targets, not
unversioned raw coordinates.

Examples:

```text
ColumnSeparator { widget_id, left_column_id, right_column_id }
RowCell { widget_id, row_id, column_id }
ScrollbarThumb { widget_id, axis }
OverlayDismissRegion { overlay_id }
FocusableWidget { widget_id }
Point { x, y, coordinate_space }
```

If an intent target resolves to coordinates, the resolved coordinate,
source probe, probe epoch, and target identity MUST be recorded in the
trace.

### 10.3 Pointer Path Policies

Pointer movement MUST use an explicit path policy.

Required pointer path policies:

```text
Instant
Linear { steps }
Jittered { steps, amplitude, seed }
EdgeAware { approach, steps }
OvershootCorrect { overshoot, correction_steps }
HoverPause { steps, hover_frames }
```

`Instant` is allowed for exact replay, shrinker minimization, and tests
that explicitly target direct coordinate placement. It MUST NOT be the
default policy for generated user-like interaction.

All non-instant path policies MUST expand into one or more
`PointerMove` primitive commands and declared frame or reconcile
boundaries.

Every pointer path expansion MUST also declare a frame policy for each
sample or sample group:

```text
SameFrameCoalesced
FrameSeparated
ThresholdCrossing
HoverSettled { frames }
ReconcileSeparated
```

The frame policy determines whether `InputFusion` may coalesce movement
samples before routing or whether each sample must be routed through the
widget tree as an observable transaction.

Coalesced movement is valid only when the trace records the primitive
sample group and declares that intermediate samples are fusion-visible
but not router-visible. Routed movement is valid only when frame or
reconcile boundaries make the sample observable to hit-testing, hover,
capture, layout, and oracles.

Pointer path expansion MUST explicitly identify threshold-crossing
samples. For drag-like intents, the trace must distinguish:

```text
approach samples
pre-down hover samples
press sample
pre-threshold drag samples
threshold-crossing sample
post-threshold drag samples
release sample
```

No generator may rely on a final endpoint alone to stand in for these
segments unless the run mode is primitive replay, shrinker
minimization, or injection-boundary testing.

### 10.4 Keyboard Navigation Policies

Keyboard navigation MUST be modeled as traversal, not only as final
focus placement.

Required keyboard navigation policies:

```text
TabOrder
ReverseTabOrder
ArrowGrid
ArrowTree
PageStep
HomeEnd
Activation
TextEntry
```

Keyboard intent expansion MUST record every `KeyDown`, `KeyUp`,
`TextInput`, frame boundary, focus transition, and authority mode
observed during traversal.

### 10.5 Intent Expansion Recording

Every expanded intent command MUST record:

```text
intent_index
intent_command
resolved_targets
path_policy
primitive_command_range
probe_epoch_before
probe_epoch_after
coverage_keys_added
```

All commands MUST carry deterministic time. Time may be represented as:

* absolute monotonic test ticks
* relative monotonic deltas
* implicit step index with deterministic conversion

Wall-clock time is forbidden in replay artifacts.

No generated primitive command may be hidden from the trace. If the
runner moves a pointer, presses a key, emits text, waits a frame,
scrolls, reconciles, or changes window state, that primitive command
MUST appear in the replay artifact.

---

## 11. Transaction Boundary

The transaction is the atomic unit of oracle evaluation.

A transaction may contain one command or a short command group where the
engine treats them as one semantic input step.

Examples:

* `PointerMove` plus one frame
* `PointerDown` plus event routing
* drag update plus reconcile
* wheel input plus scroll update
* key input plus layout reveal
* one expanded movement path step plus hover reconciliation
* one keyboard traversal step plus focus reconciliation

Intent expansion MUST NOT hide intermediate transactions. Each pointer
movement step, keyboard traversal step, hover pause, scroll step, and
frame boundary is a transaction unless the trace explicitly declares an
atomic primitive group and records why grouping is valid.

After each transaction, the runner MUST:

1. drain generated fused events
2. run event routing
3. run deferred mutation phase
4. run rebuild/layout/reconcile to the configured settle boundary
5. update animation/ticker only as declared by the fixture
6. collect probes
7. evaluate oracles
8. record coverage

---

## 12. Settle Boundary

A transaction must define when the runtime is stable enough to check.

Valid settle boundaries:

* `after_event_routing`
* `after_deferred_mutations`
* `after_layout`
* `after_scene_build`
* `after_n_frames(n)`
* `after_no_pending_layout_dirty`

The default for interaction contract fuzzing is `after_layout`.

Animations are not allowed to hide layout invalidation errors. If an
animation remains active, the settle boundary must still prove that the
non-animated authoritative geometry is stable.

---

## 13. Probe Model

A probe is a read-only semantic snapshot.

Probe rules:

* no mutation
* no persistent references to widgets
* no borrow extension beyond snapshot collection
* no layout resolution
* no self-healing
* no recomputation of authoritative geometry
* no cache priming
* no fallback coordinate synthesis
* no hidden tolerance rules

Probe data must be serializable.

Probe data must be suitable for failure packets.

Every probe snapshot MUST record its authority provenance:

```text
source_layer
source_owner
layout_epoch
frame_number
settle_boundary
snapshot_reason
```

Probe consumers MUST treat missing probe fields as unavailable data, not
permission to recompute or infer authoritative state from another layer.

---

## 14. Required Universal Probes

The runtime probe set MUST include:

```text
RuntimeProbe:
  frame_number
  layout_epoch
  intent_index
  primitive_step_index
  active_intent_command
  active_path_policy
  pending_layout_dirty_count
  focused_widget
  keyboard_authority_mode
  pointer_capture_owner
  hover_target
  incidental_target
  active_overlay_stack
  active_modal_root
  scene_node_count
  panic_or_error_events
```

```text
GeometryProbe:
  realized_widget_rects
  paint_rects
  clip_rects
  hit_test_bounds
  viewport_rects
```

```text
InputProbe:
  active_pointer_buttons
  pointer_position
  pointer_path_progress
  drag_state
  pending_coalesced_pointer_move
  pending_coalesced_scroll
```

---

## 15. Required Tabular/ListView Probes

The first implementation target is ListView/TreeView tabular
interaction.

The ListView probe MUST expose:

```text
ListViewProbe:
  widget_id
  root_rect
  header_rect
  body_rect
  footer_rect
  horizontal_scroll_offset
  vertical_scroll_offset
  resize_state
  active_resize_separator
  hovered_separator
  pointer_in_expansion_region
  columns[]
  realized_rows[]
  selected_rows
  focused_row
```

Each column entry MUST include:

```text
ColumnProbe:
  column_id
  display_index
  visible
  resizable
  reorderable
  scroll_domain
  preferred_width
  resolved_x_offset
  resolved_width
  min_width
  max_width
  separator_x_viewport
  separator_x_content
```

Each realized row entry MUST include:

```text
RealizedRowProbe:
  row_id
  data_index
  visual_index
  rect
  cells[]
```

Each cell entry MUST include:

```text
CellProbe:
  column_id
  rect
  content_state
```

TreeView must expose the same tabular probe where it shares the
tabular substrate, plus:

```text
TreeViewProbe:
  expanded_node_ids
  visible_projection
  focused_node
  selected_node_ids
  disclosure_hit_zones
  indentation_geometry
```

---

## 16. Universal Oracles

Universal oracles run after every transaction unless explicitly
disabled by fixture scope.

Oracles are read-only judges. They MUST NOT:

* mutate runtime state
* request layout or scene rebuilds
* normalize captured values before checking
* invent missing probe data
* reinterpret ownership boundaries to make a trace pass

An oracle may compute derived comparison values only from its declared
inputs, the active contract, and recorded trace metadata. Derived values
must be serialized into failure packets when they affect pass/fail.

### ORACLE-346-U1: Finite Sink Geometry

Resolved, rendered, hit-testable, realized, and submitted geometry must
be finite.

Allowed non-finite values are restricted to declarative constraint
space or explicitly unbounded abstract model space.

Non-finite values MUST NOT cross into:

* final widget rects
* paint rects
* clip rects
* hit-test bounds
* pointer interaction zones
* realized row/cell rects
* text glyph quads
* scene graph node bounds
* GPU submission buffers
* overlay anchor rects
* scrollbar geometry
* separator geometry

### ORACLE-346-U2: Geometry Ownership Boundary

No interaction transaction may mutate final layout geometry directly.

Interaction code may request mutation through declared intent paths.
Layout must remain the authority for resolved geometry.

### ORACLE-346-U3: Capture Owner Validity

If pointer capture exists, the owner widget must:

* exist
* be in the active interaction domain
* be capture-eligible
* receive the corresponding release or cancellation path

### ORACLE-346-U4: Capture Cleanup

Pointer capture MUST NOT survive pointer up, pointer cancel, or owning
widget destruction.

### ORACLE-346-U5: Focus Owner Validity

If focus exists, the focused widget must:

* exist
* be in the active focus domain
* be focusable or explicitly authorized by a focused composite

### ORACLE-346-U6: Hit-Test Bounds Coherence

The resolved hit-test target must be inside the effective hit-test
bounds and effective clip policy for that target.

### ORACLE-346-U7: Layout Settled

At the declared settle boundary, layout dirty state must match the
boundary contract.

For `after_layout`, layout-affecting dirty flags must be resolved unless
the fixture explicitly models continuous layout invalidation.

### ORACLE-346-U8: No Stale Identity

Any observed widget, row, column, or node identity must map to a live
source in the current fixture.

### ORACLE-346-U9: No Hidden Mutation From Read Probes

Collecting probes must not change:

* model versions
* layout epochs
* dirty flags
* selection state
* focus state
* capture state
* scroll offsets

### ORACLE-346-U10: Event Path Integrity

An input command must travel through the declared injection path. The
driver must record the path used.

If the path differs from the default path, the confidence packet must
state that reduced path.

### ORACLE-346-U11: Incidental Navigation Integrity

While an intent command is moving toward its target, every intermediate
hover target, hit-test target, focus target, overlay boundary, scroll
target, and capture owner must satisfy the same universal validity
oracles as the final target.

An incidental target may change runtime state only through the normal
input path. The runner MUST NOT suppress hover, enter, leave, focus,
capture, scroll, layout, or overlay side effects merely because they are
not the intended final action.

If a failure occurs during incidental navigation, the failure packet
MUST identify the active intent command, primitive command index,
path policy, incidental target, and intended final target.

---

## 17. Tabular Resize Oracles

The first binding domain oracle set is ListView/TreeView column resize.

### ORACLE-346-R1: Separator Target Stability

The column selected by separator hit-testing at arm time is the only
column allowed to receive the resize commit for that drag transaction.

### ORACLE-346-R2: Direction Monotonicity

For a resizable column with no clamp reached:

```text
dx > 0 -> resolved_width_after > resolved_width_before
dx < 0 -> resolved_width_after < resolved_width_before
dx = 0 -> resolved_width_after = resolved_width_before
```

If clamped, the result must equal the declared clamp boundary.

### ORACLE-346-R3: Width Formula

For a drag update:

```text
proposed_width = initial_width + (current_global_x - start_global_x)
```

The committed model width must equal:

```text
clamp(proposed_width, max(min_width, MIN_COL_WIDTH), max_width)
```

### ORACLE-346-R4: No Opposite-Direction First Drag

The first drag after fixture construction must obey the same direction
semantics as all subsequent drags.

This oracle specifically targets stale anchor, stale scroll, and
coordinate-basis normalization bugs.

### ORACLE-346-R5: Scroll-Aware Separator Hit-Test

For scrollable columns, separator hit-testing must use content-local X:

```text
test_x = viewport_local_x + horizontal_scroll_offset
```

For pinned columns, separator hit-testing must use viewport-local X:

```text
test_x = viewport_local_x
```

### ORACLE-346-R6: Separator Visual/Hit Coherence

The visual separator position and hit-test separator position must refer
to the same column boundary under the same scroll domain.

### ORACLE-346-R7: Header/Body/Footer Synchronization

After resize and layout, header, body, and footer must consume the same
resolved column geometry table.

They must not compute independent widths.

### ORACLE-346-R8: Unaffected Columns Stable

Columns not governed by the active resize policy must retain their
preferred and resolved widths unless a declared policy explicitly
redistributes width.

The default policy is no implicit redistribution.

### ORACLE-346-R9: Active Resize State Cleanup

After pointer up:

* resize state is idle
* active resize separator is none
* pointer capture is released
* horizontal scroll freeze is false
* hover state is either recomputed or cleared by contract

### ORACLE-346-R10: Cancel Restores Initial Width

If Escape or explicit cancel occurs during resize, the controlled column
must return to its width at arm time, subject only to deterministic
rounding specified by the column model.

### ORACLE-346-R11: Inverse Drag Restoration

For a non-clamped column:

```text
drag +dx
settle
drag -dx
settle
```

must restore the original resolved width within the declared rounding
tolerance.

Default tolerance is exact equality for logical widths unless the
contract names a pixel-snap boundary.

### ORACLE-346-R12: Repeated Drag Consistency

For identical initial geometry and identical drag delta, repeated resize
transactions must produce identical results.

---

## 18. Scroll Oracles

### ORACLE-346-S1: Scroll Bounds

Scroll offsets must remain within the declared scroll domain bounds.

Overscroll attempts may be represented only through an explicit
overscroll state. They must not mutate canonical scroll offset beyond
allowed bounds.

### ORACLE-346-S2: Scroll Is Transform Only

Horizontal scroll must not mutate column widths or resolved x offsets.

### ORACLE-346-S3: Header/Body/Footer Scroll Lockstep

For tabular views, header, body, and footer must project the same
horizontal scroll offset over the shared geometry table.

### ORACLE-346-S4: Virtualization Projection Validity

Realized rows after scroll must map to valid data indices and stable
row identities.

### ORACLE-346-S5: Scroll Then Resize Coherence

After horizontal scroll, column separator hit-testing and resizing must
still target the column represented by the visible separator.

---

## 19. Selection and Focus Oracles

### ORACLE-346-F1: Selection Validity

Selection may contain only live row, node, or item identities from the
current projection domain.

### ORACLE-346-F2: Focus/Selection Separation

Changing focus must not implicitly corrupt selection.

Changing selection must not implicitly move focus unless the widget
contract declares coupled focus-selection behavior.

### ORACLE-346-F3: Keyboard Authority Mode

Keyboard events must be interpreted according to the active authority
mode. Navigation mode and capture mode must not both consume the same
event as authoritative.

### ORACLE-346-F4: Focus Survival Under Virtualization

If a focused item scrolls out of realization but remains semantically
valid, focus state must remain valid by identity, not by stale realized
cell instance.

---

## 20. Overlay and Modal Oracles

### ORACLE-346-O1: Modal Boundary

When a modal root owns interaction, underlying widgets must not receive
pointer or keyboard transactions outside authorized escape/dismissal
paths.

### ORACLE-346-O2: Overlay Capture Supremacy

Overlay capture must dominate underlying widget capture.

### ORACLE-346-O3: Overlay Anchor Validity

Overlay anchor rects must be finite and must map to a live widget or
declared virtual anchor.

### ORACLE-346-O4: Dismissal Cleanup

After overlay dismissal, focus, capture, hover, and pending interaction
state must not reference destroyed overlay widgets.

---

## 21. Generator Model

Generators must produce structurally valid transactions for a declared
domain.

Generators MUST produce intent traces by default. The runner expands
those intent traces into primitive traces before injection.

Generator classes:

```text
PointerGenerator
PointerPathGenerator
WheelGenerator
KeyboardGenerator
KeyboardNavigationGenerator
TextGenerator
ResizeGenerator
ScrollGenerator
SelectionGenerator
FocusGenerator
OverlayGenerator
IncidentalInteractionGenerator
FixtureMutationGenerator
```

Generators MUST be coverage-aware.

Pointer path generators MUST vary route shape, approach side, velocity
class, hover pause, overshoot, and correction behavior while preserving
deterministic replay.

Keyboard navigation generators MUST vary traversal policy, modifier
state, authority mode, focus scope, and activation timing while
preserving deterministic replay.

Incidental interaction generators MUST deliberately route through
non-final targets such as separators, cells, overlay edges, scrollbars,
clipped regions, empty space, stale-prone geometry, and focusable
children before performing or abandoning the intended action.

Generators MUST respect widget affordances:

* do not type into non-text controls unless testing rejection
* do not resize non-resizable columns unless testing rejection
* do not drag invisible separators unless testing hit-test exclusion
* do not interact outside active modal root unless testing modal boundary

Invalid-action generation is allowed only when an oracle explicitly
defines expected rejection behavior.

---

## 22. Seed Model

Every run must record:

* root seed
* per-generator seeds
* fixture seed
* shrink seed, if separate
* deterministic scheduler seed, if used

The same artifact must replay the same trace.

---

## 23. Coverage Model

Fixed transaction count is not a valid primary halt condition.

Coverage keys are semantic, not line-based.

Required universal coverage dimensions:

```text
focused_widget_class
capture_owner_class
keyboard_authority_mode
hover_target_class
incidental_target_class
intent_command_class
input_command_class
pointer_path_policy
keyboard_navigation_policy
event_phase_path
dirty_state_transition
layout_epoch_transition
scroll_position_class
overlay_state_class
virtualization_state_class
```

Required pointer path exposure dimensions:

```text
path_sample_count_class
path_segment_class
path_frame_policy
coalesced_move_class
threshold_crossing_class
crossed_hover_target_class
crossed_separator_class
capture_transition_class
incidental_route_target_class
endpoint_teleportation_mode
```

Pointer path exposure coverage records what the generated movement
actually exposed to the runtime. A run that reaches the right endpoint
without covering intermediate path exposure does not satisfy user-like
interaction coverage.

Required chaos pressure coverage dimensions:

```text
chaos_surface_class
primitive_command_class
primitive_burst_class
pointer_button_state_class
pointer_position_region_class
pointer_velocity_class
pointer_acceleration_class
pointer_curvature_class
pointer_jitter_class
edge_corner_pressure_class
drag_threshold_state_class
drag_duration_class
wheel_burst_class
key_chord_class
text_input_class
window_resize_class
scale_factor_transition_class
frame_gap_class
settle_attempt_class
unsupported_domain_class
hard_failure_detector_class
```

Chaos pressure coverage records which primitive pressure patterns and
runtime boundary classes were exercised. It is valid for corpus
selection, halting, and failure-discovery reporting. It is not valid as
a semantic confidence frontier unless a declared contract/user-like
profile also ran.

Required tabular coverage dimensions:

```text
column_scroll_domain
separator_region
resize_state_transition
resize_delta_class
clamp_boundary_class
horizontal_scroll_class
realized_row_range_class
selection_state_class
focus_row_state_class
```

Transition coverage key:

```text
(abstract_before_state, command_class, abstract_after_state)
```

The confidence packet must include:

* total unique state keys
* total unique transition keys
* new keys discovered in final window
* saturation window size
* generator classes exercised
* oracle classes exercised

---

## 24. Halting Model

The fuzzer must support at least three modes.

### 24.1 CI Smoke Mode

CI smoke mode must terminate quickly.

Primary halt:

* semantic coverage saturation over a bounded window

Secondary halt:

* wall-clock or deterministic transaction budget

The report must say which halt fired.

### 24.2 Developer Deep Mode

Developer deep mode runs until:

* failure
* manual stop
* configured time budget
* configured coverage saturation threshold

Passing deep mode produces a confidence packet.

### 24.3 Continuous Mode

Continuous mode keeps a persistent corpus and reports only:

* new coverage
* new minimized failures
* coverage regressions
* oracle regressions

Continuous mode has no global "complete" claim.

### 24.4 Chaos / Bombardment Mode

Chaos/bombardment mode runs seeded primitive command streams against a
declared chaos surface.

It is designed to discover hard runtime failures, not to prove semantic
interaction contracts.

Required properties:

* the surface must be deterministic and declared before execution
* the command stream must be reproducible from recorded seed,
  RNG algorithm, RNG version, generator version, and stream policy
* every generated primitive command must be recorded before injection
* every frame and reconcile boundary must be recorded
* pointer movement must preserve intermediate movement samples
* endpoint-only teleportation is allowed only in a named
  direct-injection submode
* the hard-failure oracle set must be enabled
* unsupported commands or unsupported domains must be reported, not
  silently skipped

Required primitive families:

* pointer move
* pointer down and pointer up
* wheel pixels and wheel lines
* key down and key up
* text input
* frame
* reconcile
* window size changes
* scale factor changes

Required pointer pressure patterns:

* random walk
* velocity bursts
* acceleration bursts
* jitter
* edge and corner bias
* drag-threshold crossing
* long drag
* hover pause
* press-drag-release sequences
* wheel bursts under pointer
* movement through stale, clipped, and scrolled regions when present

Hard-failure oracles required for chaos mode:

* panic, crash, or unwind escape
* timeout, hang, or no-progress loop
* non-finite value crossing a final widget, layout, render, hit-test,
  scene, or GPU submission sink
* pointer capture surviving pointer-up, cancel, surface teardown, or
  domain loss
* dirty layout at settle boundary unless continuous invalidation is
  explicitly declared for the surface
* event router contradiction between handled status, capture owner,
  focus owner, hovered target, and routed target
* hit-test, layout, render, focus, capture, scroll, or identity
  authority violation
* replay nondeterminism
* unsupported command/domain skip without report

Chaos mode halts on:

* hard failure
* deterministic primitive command budget
* deterministic transaction budget
* configured wall-clock budget in developer mode
* no-new-coverage window
* corpus saturation

Chaos pass output is limited to:

* executed budget
* seed set
* surface set
* hard-failure oracle set
* coverage summary
* corpus deltas
* unsupported domains
* no-failure statement scoped to the executed chaos budget

Chaos pass output MUST NOT be a confidence packet unless a separate
bounded contract profile was also executed and reported.

### 24.5 Timeout and No-Progress Detection

Timeout and no-progress detection MUST be declared in the interaction
fuzz profile. A run with implicit timeout defaults is invalid.

Required profile fields:

```text
timeout_policy:
  per_transaction_timeout_ms
  run_timeout_ms
  settle_iteration_budget
  frame_progress_budget
  no_progress_frame_budget
  no_progress_settle_budget
  monotonic_clock_seed
```

All values must be positive integers.

The deterministic progress marker is:

```text
progress_marker:
  command_index
  frame_index
  layout_epoch
  scene_epoch
  dirty_state_hash
  deferred_mutation_count
  capture_owner_class
  focus_owner_class
  routed_target_class
```

`timeout` fires when monotonic elapsed time exceeds either
`per_transaction_timeout_ms` for the active transaction or
`run_timeout_ms` for the active run.

`no_progress` fires when pending work exists and the progress marker is
unchanged for either `no_progress_frame_budget` consecutive frames or
`no_progress_settle_budget` consecutive settle iterations.

`settle_budget_exhausted` fires when settle attempts exceed
`settle_iteration_budget` before the declared settle boundary is
reached.

Timeout and no-progress failures MUST record the active command,
progress marker history, configured budget, observed count, and last
runtime owner that made progress.

---

## 25. Confidence Packet

A passing contract/user-like run MUST emit a confidence packet.

A passing chaos/bombardment run MUST NOT emit a confidence packet unless
it also executed a declared bounded contract/user-like profile.

Minimum fields:

```text
confidence_packet:
  run_id
  run_profile_id
  run_profile_hash
  halcyon_version
  git_revision
  fixture_set
  generator_set
  oracle_set
  seed_set
  injection_path
  settle_boundary
  transaction_count
  coverage_summary
  saturation_summary
  elapsed_time
  failure_count
  skipped_oracles
  disabled_generators
  out_of_scope_domains
  timeout_policy
  visual_diagnostics_status
  constitution_boundary_profile
  constitution_boundary_check_status
  positive_claim
```

The `positive_claim` must use bounded language:

```text
No invariant violations found across the declared fixture/generator/oracle
frontier for this run.
```

Forbidden positive claims:

* "ListView is correct"
* "TreeView is bug-free"
* "all interactions work"
* "no bugs exist"

---

## 26. Failure Packet

Every failure MUST produce one failure packet.

Minimum fields:

```text
failure_packet:
  failure_id
  run_profile_id
  run_profile_hash
  invariant_id
  invariant_name
  severity
  fixture_id
  root_seed
  shrink_seed
  replay_command
  injection_path
  settle_boundary
  failing_intent_index
  failing_transaction_index
  failing_primitive_step_index
  active_intent_command
  command_that_failed
  intended_target
  resolved_target
  incidental_target
  path_policy
  intent_trace
  primitive_trace
  shrunk_trace
  unshrunk_trace_reference
  pre_state
  post_state
  expected_state
  observed_state
  layer_classification
  ownership_chain
  coverage_key
  timeout_policy
  progress_marker_history
  visual_trace_artifacts
  constitution_boundary_profile
  constitution_boundary_check_status
  related_specs
  promotion_target
```

The packet MUST be enough for a developer to replay the failure without
manual reconstruction.

When the failing source is chaos/bombardment mode, intent-specific
fields that do not apply MUST be present with an explicit
`not_applicable` value. The packet MUST additionally include:

```text
chaos_failure_fields:
  chaos_surface_id
  chaos_surface_state_id
  chaos_mode
  chaos_stream_generator
  chaos_stream_policy
  rng_algorithm
  rng_version
  rng_seed
  primitive_stream_range
  failing_command_index
  failure_detector
  hard_failure_class
  timeout_budget
  replay_determinism_status
  initial_state_snapshot
  final_state_snapshot
  unsupported_domains
```

---

## 27. Shrinking

The shrinker must reduce a failing trace while preserving the failure.

Shrink dimensions:

* remove intent commands
* remove transactions
* remove commands inside transaction groups
* reduce intent command parameters
* reduce pointer path policy complexity
* reduce pointer path step count
* reduce pointer deltas
* reduce scroll deltas
* reduce keyboard navigation count
* reduce key modifier combinations
* reduce fixture size
* reduce row count
* reduce column count
* reduce overlay depth
* reduce animation/frame count

The shrinker MUST NOT produce an invalid trace.

The shrinker MUST NOT bypass the original injection path.

The shrinker MUST NOT replace a user-like path with `Instant` unless
the minimized artifact records the layer reduction and preserves a
primitive trace that reproduces the same failure.

The shrinker MUST record if the minimized failure differs in layer
classification from the original failure.

Chaos shrink MUST reduce the primitive command stream while preserving:

* the same injection boundary
* the same hard-failure class
* the same failure detector or an explicitly recorded detector
  equivalence
* deterministic replay from the shrunk primitive trace
* all required chaos failure packet fields

---

## 28. Replay

A replay artifact must run without randomness unless explicitly asked to
re-randomize.

Replay must support:

* exact intent replay
* exact primitive replay
* exact trace replay
* shrunk trace replay
* primitive-only replay of an expanded intent trace
* intent expansion diff against stored primitive trace
* replay with oracle-only mode
* replay with probe dump
* replay with frame snapshots, if rendering is available

Replay must fail if:

* fixture cannot be built
* required probe is unavailable
* required oracle is unavailable
* versioned artifact schema is unsupported

Replay must not silently skip missing requirements.

### 28.1 Interaction Fuzz Profile and Runner Contract

Every run MUST be created from an interaction fuzz profile. A run
without a complete profile is invalid.

Required profile schema:

```text
InteractionFuzzProfile:
  schema_version
  profile_id
  mode
  artifact_dir
  injection_path
  settle_boundary
  fixture_ids
  chaos_surface_ids
  generator_ids
  chaos_stream_generator_ids
  oracle_ids
  hard_failure_oracle_ids
  root_seed
  seed_set
  rng_algorithm
  rng_version
  command_budget
  transaction_budget
  coverage_window
  timeout_policy
  visual_diagnostics_policy
  constitution_boundary_profile
  disabled_domains
  expected_unsupported_domains
```

Valid `mode` values:

```text
contract_smoke
contract_deep
continuous
chaos_smoke
chaos_deep
replay
shrink
negative_validation
profile_diff
```

Runner operations required by this SPEC:

```text
run(profile) -> failure_packet | confidence_packet | chaos_report
replay(artifact) -> replay_report
shrink(failure_packet) -> failure_packet
validate_negative(profile) -> negative_validation_report
render_visual_trace(artifact) -> visual_trace_report
```

If a CLI is provided, it MUST be a thin wrapper over these operations
and MUST NOT add alternate behavior, hidden defaults, or OS automation.

Required artifact layout:

```text
artifact_dir/
  run-profile.json
  run-report.json
  confidence-packet.json
  chaos-report.json
  failure-packet.json
  replay-report.json
  negative-validation-report.json
  traces/intent-trace.json
  traces/primitive-trace.json
  traces/shrunk-trace.json
  diagnostics/visual-trace.json
  diagnostics/pointer-path.svg
  diagnostics/command-timeline.json
  diagnostics/frame-sequence/
```

Only artifacts produced by the executed mode are required. Missing
mode-required artifacts invalidate the run.

The profile hash MUST be recorded in every packet and report. Replay
MUST reject an artifact whose stored profile hash does not match the
profile used for replay unless the replay is explicitly run in
`profile_diff` diagnostic mode.

---

## 29. Promotion to Regression Tests

A minimized failure that reveals a real bug must be promotable to a
normal regression test.

Promotion target rules:

* black-box public behavior -> `tests/`
* internal white-box invariant -> `src/tests/`

No other test locations are allowed.

The promoted test must:

* include the minimized trace or equivalent deterministic harness call
* assert the violated invariant
* avoid sleeps and wall-clock timing
* not depend on external services
* not require manual window interaction

Stub reproductions are forbidden by SPEC-HAL-129.

---

## 30. Fixture Model

Fixtures must be deterministic.

Fixture fields:

```text
Fixture:
  id
  description
  root_widget_type
  window_size
  scale_factor
  theme
  data_set
  initial_focus
  initial_scroll
  initial_selection
  enabled_generators
  enabled_oracles
  settle_boundary
```

Fixture state may be generated from a seed, but the generated result
must be serialized into failure packets.

### 30.1 Chaos Surface Model

Chaos surfaces must be deterministic.

Chaos surface fields:

```text
ChaosSurface:
  id
  description
  root_surface_kind
  construction_entrypoint
  build_identity
  window_size
  scale_factor
  theme
  clock_seed
  initial_focus
  initial_scroll
  initial_selection
  initial_state_snapshot
  allowed_primitive_families
  enabled_hard_failure_oracles
  disabled_domains
  expected_unsupported_domains
  settle_boundary
  command_budget
  transaction_budget
  coverage_window
  timeout_policy
  corpus_policy
  visual_diagnostics_policy
  constitution_boundary_profile
```

`construction_entrypoint` must name a deterministic in-process builder.
It must not open network connections, inspect user display state, move
the OS cursor, call browser automation, or rely on external services.

`initial_state_snapshot` must be recorded before the first primitive
command is injected.

`disabled_domains` and `expected_unsupported_domains` must be reported
in every chaos report. Any unsupported domain not listed before the run
is a reportable failure.

`constitution_boundary_profile` must declare the runtime owner for input
fusion, event routing, capture, focus, layout, scene build, rendering,
scroll, identity, probes, oracles, shrinking, replay, reporting, and
visual diagnostics.

---

## 31. First Fixture Set

The first required fixture set is tabular interaction:

### FIXTURE-346-LV1: ListView Basic Columns

* 5 visible columns
* fixed widths
* more total width than viewport
* vertical and horizontal scroll enabled
* non-empty rows
* header/body/footer present where supported

### FIXTURE-346-LV2: ListView Scrolled Horizontally

Same as LV1, but starts with horizontal scroll offset in the middle of
the scrollable domain.

### FIXTURE-346-LV3: ListView Boundary Separators

Exercises:

* first separator
* last visible separator
* pinned/scrollable boundary
* scrollable/pinned-right boundary if supported

### FIXTURE-346-LV4: ListView Clamp Boundaries

Contains columns near min and max width.

### FIXTURE-346-LV5: ListView Virtualized Body

Enough rows to force virtualization.

### FIXTURE-346-TV1: TreeView Basic Projection

TreeView over hierarchical data with expandable nodes and shared
tabular projection.

---

## 32. First Generator Set

The first implementation MUST include:

### GEN-346-RSZ1: Separator Resize Transaction

1. move pointer to separator center
2. pointer down
3. pointer move past drag threshold
4. pointer move by generated `dx`
5. pointer up
6. settle

### GEN-346-RSZ2: Resize While Horizontally Scrolled

Same as RSZ1, with non-zero horizontal scroll.

### GEN-346-RSZ3: First Drag vs Subsequent Drag

Performs two equivalent drags from equivalent starting geometry and
checks direction consistency.

### GEN-346-RSZ4: Inverse Drag

Performs +dx and -dx restoration.

### GEN-346-SCR1: Wheel Scroll Then Resize

Scrolls horizontally or vertically, then runs separator resize.

### GEN-346-FOC1: Focus/Keyboard Navigation Smoke

Exercises focus movement and row navigation without pointer interaction.

### GEN-346-PTH1: Incidental Pointer Path To Resize

Moves the pointer from outside the ListView to a target separator using
non-instant path policies before resize.

The path MUST cross at least one non-final hit-test class where possible,
such as a row cell, header cell, empty body area, scrollbar zone, or
adjacent separator.

### GEN-346-PTH2: Overshoot Then Correct To Separator

Moves past the intended separator, pauses or frames once, then corrects
back to the separator before drag.

### GEN-346-PTH3: Hover Pause Before Action

Moves to a hover-sensitive target, waits declared frames, then continues
to the final intended target.

### GEN-346-KBD1: Traversal Before Activation

Moves focus through one or more intermediate targets before activating
or typing into the final target.

---

## 33. Report Classification

Failures must be classified into one primary layer:

```text
intent_expansion
navigation_path
incidental_interaction
input_fusion
event_routing
capture
hit_test
coordinate_space
layout_invalidation
geometry_resolution
scroll_projection
virtualization_projection
selection
focus
overlay
render_projection
probe_contract
oracle_contract
fixture_contract
```

The classifier may attach secondary layers, but the primary layer is
required.

---

## 34. Severity

Default severity:

* P0: panic, crash, undefined replay, non-deterministic replay
* P1: invariant violation that corrupts authority, capture, focus,
  geometry, identity, or data projection
* P2: wrong interaction result with bounded local effect
* P3: report-only issue, missing coverage, weak probe, or skipped
  oracle

The resize opposite-direction bug class is P2 by default. It becomes P1
if it corrupts geometry authority, identity, capture, or layout state.

---

## 35. Determinism Requirements

The fuzzer must control:

* random seeds
* RNG algorithm and version
* monotonic time
* frame count
* window size
* scale factor
* fixture data
* generator order
* command order
* oracle order
* shrink order

The fuzzer must record:

* all commands
* all generated coordinates
* all generated command parameters
* all state snapshots needed by oracles
* all fixture-derived values used by generators
* all chaos surface-derived values used by chaos stream generators

Nondeterministic failures are still failures, but they must be labeled
`replay_nondeterministic` until reduced.

---

## 36. Tolerance Policy

Tolerance is forbidden by default.

An oracle may use tolerance only when the owning SPEC declares a
rounding, snapping, or floating-point boundary.

Tolerance must be named in the oracle:

```text
tolerance_source: pixel_snap | physical_pixel | text_metric | animation_sample
tolerance_value: ...
```

Silent approximate equality is forbidden.

---

## 37. Infinite and Unbounded Spaces

Halcyon may contain unbounded or infinite abstract domains.

The fuzzer must distinguish:

Allowed non-finite/unbounded domains:

* constraints
* abstract canvas/world space
* virtual content model
* lazy model ranges
* scrollable domain before viewport projection, if declared

Forbidden non-finite sinks:

* final layout rect
* paint rect
* clip rect
* hit-test rect
* realized cell rect
* separator geometry
* pointer interaction zone
* overlay anchor rect
* scene graph bound
* GPU submission geometry

The oracle checks phase boundary crossings, not abstract model values.

---

## 38. Corpus Management

The fuzzer maintains a corpus of traces that contributed new semantic
coverage.

Corpus entries must include:

* fixture id
* chaos surface id when generated by chaos mode
* generator id
* chaos stream generator id when generated by chaos mode
* seed
* RNG algorithm and version
* shrunk trace if failure
* shrunk primitive trace if chaos failure
* hard-failure class if chaos failure
* coverage keys gained
* oracle set used
* hard-failure oracle set used when generated by chaos mode

The corpus must not store traces that require external manual state.

---

## 39. CI Integration

CI must run a bounded smoke profile.

Minimum CI profile:

* deterministic fixture set
* fixed seed set
* coverage saturation window
* strict failure on invariant violation
* confidence packet artifact

CI must not run unbounded continuous mode.

CI may run fewer generators than developer deep mode, but the confidence
packet must state the reduced frontier.

CI MAY run a bounded chaos smoke profile.

CI chaos smoke MUST be reported separately from contract/user-like
confidence smoke. Passing CI chaos smoke emits a chaos report and corpus
summary, not a confidence packet.

---

## 40. Developer Workflow

Expected workflow:

1. Run smoke fuzzer.
2. If failure appears, inspect failure packet.
3. Replay shrunk trace.
4. Fix root cause.
5. Promote shrunk trace to regression test.
6. Re-run smoke fuzzer.
7. Preserve confidence packet as evidence.

The workflow must not require reading 500-event logs.

---

## 41. Visual Diagnostics

Visual diagnostics are required for failure packets when the active run
has scene or frame capture capability.

Visual diagnostics are optional only when the run is headless or
rendering is unavailable. A profile may disable visual capture only by
declaring one of those two conditions. The packet or report MUST then
record:

```text
visual_diagnostics_status:
  unavailable
  reason
  profile_policy
```

Allowed visual artifacts:

* pre-state screenshot
* post-state screenshot
* overlay showing command path
* highlighted target widgets
* separator/rect overlays
* frame sequence for minimized trace
* pointer path SVG generated from primitive trace
* command timeline generated from primitive trace
* capture/focus/hit-test target timeline generated from probes
* dirty/settle marker timeline generated from runtime progress markers

Visual diagnostics MUST be derived from:

* recorded intent trace
* recorded primitive trace
* probe snapshots
* runtime progress markers
* scene output produced by Halcyon
* rendered frames produced by Halcyon

Visual diagnostics MUST NOT be derived from:

* OS cursor position
* browser automation
* external screenshot comparison
* AI judgment of rendered output
* inferred widget semantics from pixels

Visual diagnostics must reference the semantic or hard-failure detector
that fired. They are not the primary oracle.

### 41.1 Visual Confirmation Profile

The runner MUST support a visual confirmation profile for developer
inspection.

The visual confirmation profile MUST emit:

```text
visual_confirmation:
  run_profile_id
  profile_hash
  replay_command
  primitive_trace
  command_timeline
  pointer_path_overlay
  target_timeline
  frame_sequence_status
  failure_or_no_failure_status
```

Visual confirmation MUST replay an existing artifact or execute a
declared profile. It MUST NOT create a new unrecorded run.

---

## 42. Reporting Format

Reports must support both human and machine consumers.

Human report:

* invariant name
* short failure statement
* replay command
* shrunk trace summary
* pre/post relevant state
* primary layer classification
* visual diagnostics status

Machine report:

* complete serialized failure or confidence packet
* complete serialized chaos report when chaos mode ran
* complete serialized negative validation report when validation ran
* schema version
* all replay data
* all oracle metadata
* all profile and Constitution boundary metadata

### 42.1 Negative Validation Report

Every required oracle, hard-failure detector, replay validator, shrinker
validator, reporter validator, and Constitution boundary check MUST have
at least one negative validation case.

Required negative validation classes:

```text
negative_validation_classes:
  semantic_oracle_known_bad_state
  finite_sink_non_finite_sample
  capture_leak_sample
  dirty_settle_violation_sample
  router_contradiction_sample
  timeout_sample
  no_progress_sample
  panic_escape_surface
  replay_nondeterminism_artifact
  unsupported_domain_artifact
  missing_required_artifact
  confidence_overclaim_artifact
  visual_oracle_substitution_attempt
  constitution_authority_bypass_attempt
```

Required report fields:

```text
negative_validation_report:
  run_profile_id
  profile_hash
  validator_id
  validation_class
  expected_failure_class
  observed_failure_class
  detector_that_fired
  artifact_used
  authority_boundary_status
  passed
```

If negative validation fails for an oracle, detector, validator, or
boundary check, any run relying on that component is invalid.

Negative validation artifacts MUST be deterministic and local. They MUST
NOT require network access, live display state, OS cursor movement,
browser automation, wall-clock sleeps, or manual input.

---

## 43. Schema Versioning

All artifacts must carry:

```text
schema: halcyon.interaction_fuzz.v1
```

Schema changes must be additive unless explicitly versioned.

Replay must reject unknown required fields.

---

## 44. Security and Isolation

The fuzzer must not:

* open network connections
* use system cursor automation
* use browser automation
* require external services
* write outside configured artifact directories
* depend on user-specific display state
* read live OS cursor position as semantic input
* delegate focus, capture, routing, layout, modality, draw order, or
  frame-loop semantics to the OS

Artifacts must be deterministic and local.

---

## 45. Forbidden Behaviors

### FORBID-346-1: Random Clicking Without Oracle

Generating arbitrary UI events without declared invariants is forbidden.

Declared chaos/bombardment mode with enabled hard-failure oracles is not
random clicking without an oracle.

### FORBID-346-2: Screenshot-Only Failure

A screenshot alone is not a failure packet.

### FORBID-346-3: Fixed Count as Completion Claim

`500 iterations passed` is not a confidence claim.

Transaction count may be a budget, not proof.

### FORBID-346-4: Direct Widget Mutation

The fuzzer must not mutate widget internals to simulate interaction.

### FORBID-346-5: Unreplayable Failure

A failure that cannot be replayed must be labeled incomplete until
captured as a replayable artifact.

### FORBID-346-6: Silent Oracle Skip

Skipping an oracle without reporting it invalidates the confidence
packet.

### FORBID-346-7: Self-Healing Probe

A probe must not resolve geometry, clear state, recompute layout, or
repair stale data.

### FORBID-346-8: Hidden Tolerance

Approximate checks without a named tolerance source are forbidden.

### FORBID-346-9: Network-Dependent Smoke Test

Core fuzzing and smoke testing must be in-process and local.

### FORBID-346-10: Positive Overclaim

The subsystem must not claim global correctness.

### FORBID-346-11: Authority Laundering

The fuzzer must not move behavior across ownership boundaries to make a
trace pass. A generator, probe, oracle, shrinker, or reporter may not
perform work owned by `InputFusion`, `EventRouter`, widgets, layout,
rendering, scroll, focus, capture, or identity systems.

### FORBID-346-12: Post-Setup Harness Mutation

After the first transaction begins, fixture data and widget state must
not be mutated through harness handles. Any required mutation must be
represented as an input transaction or as a new fixture run.

### FORBID-346-13: Chaos Positive Claim

Chaos/bombardment mode must not claim app correctness, widget
correctness, or interaction confidence. It may report only failures,
absence of detected hard failures inside the executed budget, coverage
deltas, and corpus deltas.

### FORBID-346-14: Unrecorded Randomness

Every random choice must be reproducible from recorded RNG algorithm,
RNG version, seed, generator version, and deterministic input state, or
it must be stored explicitly in the artifact.

### FORBID-346-15: Visual Oracle Substitution

Visual diagnostics must not replace semantic or hard-failure oracles.
Screenshots, frame captures, path overlays, or visual traces may explain
a failure, but they cannot define whether the run passed or failed.

### FORBID-346-16: Hidden Profile Defaults

The runner must not invent seeds, budgets, timeout thresholds, coverage
windows, disabled domains, unsupported domains, artifact directories, or
diagnostics policies outside the interaction fuzz profile.

### FORBID-346-17: Live-State Fault Injection

Negative validation must not mutate live runtime state after fixture or
surface setup. Known-bad conditions must be represented by declared
test-only surfaces, synthetic oracle inputs, invalid artifacts, or input
transactions.

### FORBID-346-18: Diagnostic Authority Bleed

Visual traces, reports, replay tools, shrinkers, and validators must not
drive focus, capture, routing, layout, rendering, scroll, identity, or
widget mutation.

---

## 46. Invariants

### INV-346-1: Same Runtime Path

Default generated interactions must enter the same internal input path
as platform input after the selected injection boundary.

### INV-346-2: Deterministic Replay

Every failure must replay from its artifact.

### INV-346-3: Probe Read-Only Law

Probe collection must not mutate runtime state.

### INV-346-4: Oracle Declaration Law

Every assertion must reference a named oracle.

### INV-346-5: Coverage-Bound Confidence

A passing run must emit coverage metadata and a bounded positive claim.

### INV-346-6: Shrink Preservation

Shrinking must preserve the failing oracle.

### INV-346-7: Fixture Serialization

Failure packets must contain or reference exact fixture state sufficient
for replay.

### INV-346-8: Intent Expansion Recording

Every generated intent command must record its expanded primitive
command range.

### INV-346-9: Incidental Effects Are Observable

Intermediate hover, focus, hit-test, scroll, capture, overlay, and
layout effects caused while moving toward an intended action must be
observable through probes and checked by declared oracles.

### INV-346-10: User-Like Sampled Path Law

Generated interaction fuzzing must model pointer movement as a
deterministic sampled path through intermediate states. Endpoint
teleportation is forbidden except for exact primitive replay, shrinker
minimization, or injection-boundary tests.

### INV-346-11: Frame Policy Visibility

Every generated pointer path must declare which movement samples are
same-frame coalesced and which are frame-separated or
reconcile-separated. Coverage and oracle records must distinguish
fusion-visible samples from router-visible samples.

### INV-346-12: No Silent Out-of-Scope

Disabled generators, skipped oracles, and unsupported domains must be
listed in the confidence packet.

### INV-346-13: Promotion Path

Every confirmed runtime bug found by fuzzing must be promotable to a
normal regression test.

### INV-346-14: Authority Respect

The fuzzer may observe deep state but may not write around the runtime
authority model.

### INV-346-15: Constitution Boundary Preservation

Every generated transaction, probe snapshot, oracle evaluation, shrink
step, replay, and report must preserve SPEC-HAL-002 ownership and phase
boundaries. Any boundary narrowing must be declared as a reduced test
mode and cannot produce a full confidence claim.

### INV-346-16: Chaos Replayability

Every chaos failure must replay from the recorded primitive trace
through the same injection boundary, or be labeled P0
`replay_nondeterministic` with all observed divergence data.

### INV-346-17: Chaos Hard-Failure Oracle Presence

A chaos/bombardment run without the enabled hard-failure oracle set is
invalid.

### INV-346-18: Chaos Does Not Produce Full Confidence

A passing chaos/bombardment run cannot emit a full confidence packet or
global positive interaction claim.

### INV-346-19: Profile Completeness

Every run, replay, shrink, visual trace render, and negative validation
must reference a complete interaction fuzz profile and matching profile
hash.

### INV-346-20: Negative Validation Gate

An oracle, hard-failure detector, replay validator, shrinker validator,
reporter validator, or Constitution boundary check cannot contribute to
a passing report until its negative validation case passes.

### INV-346-21: Visual Trace Non-Authority

Visual diagnostics must be generated from recorded traces and runtime
observations, and must not determine pass/fail status.

### INV-346-22: Declarative Timeout Law

Timeout, no-progress, and settle-budget failures must be derived from
the declared timeout policy and recorded progress marker history.

### INV-346-23: Chaos Coverage Is Pressure Only

Chaos pressure coverage may drive corpus selection and chaos halting,
but it cannot produce semantic confidence without a declared
contract/user-like profile.

### INV-346-24: Constitution Audit Gate

Every run must record the Constitution boundary profile and boundary
check status. A failed boundary check invalidates the run, even if no
interaction oracle fires.

---

## 47. Acceptance Criteria

This SPEC is complete when:

1. An in-process interaction driver exists.
2. The driver can emit pointer move/down/up, wheel, key, text, frame,
   and reconcile commands.
3. The default driver path enters through `RawInputEvent` and
   `InputFusion`.
4. Intent commands exist above primitive commands.
5. Intent commands expand into recorded primitive command ranges.
6. Non-instant pointer path policies exist and replay deterministically.
7. Pointer path expansion records frame policy for every sample or
   sample group and distinguishes coalesced samples from routed samples.
8. Pointer path coverage records sample counts, path segments, frame
   policies, threshold crossings, crossed targets, capture transitions,
   and endpoint teleportation mode.
9. Keyboard navigation intents exist and replay deterministically.
10. Incidental navigation effects are probed and oracle-checked.
11. A deterministic fixture registry exists.
12. ListView fixture LV1 exists.
13. ListView fixture LV2 exists with horizontal scroll.
14. ListView resize generator RSZ1 exists.
15. ListView resize generator RSZ2 exists.
16. Pointer path generator PTH1 exists.
17. Keyboard traversal generator KBD1 exists.
18. Universal finite sink geometry oracle exists.
19. Capture owner validity oracle exists.
20. Layout settled oracle exists.
21. Incidental navigation integrity oracle exists.
22. ListView column probe exposes resolved widths and separator
    positions.
23. Resize direction monotonicity oracle exists.
24. Scroll-aware separator hit-test oracle exists.
25. First-drag/subsequent-drag consistency oracle exists.
26. Failure packet schema records intent trace and primitive trace.
27. Confidence packet schema records exercised path policies and
    incidental target classes.
28. Shrunk trace replay exists.
29. CI smoke profile exists.
30. Constitution boundary checks exist for generator, probe, oracle,
    shrinker, replay, and reporter responsibilities.
31. A passing CI smoke run emits a bounded confidence packet.
32. Chaos/bombardment mode exists as a distinct failure-discovery mode.
33. Chaos surfaces are declared, deterministic, and replayable.
34. Chaos stream generation records RNG algorithm, RNG version, seed,
    generator version, stream policy, generated coordinates, command
    parameters, frame boundaries, and reconcile boundaries.
35. Chaos pointer movement preserves intermediate samples except in a
    declared direct-injection submode.
36. Chaos hard-failure oracle set exists.
37. Chaos failures emit replayable failure packets with surface id,
    failure detector, hard-failure class, failing command index, RNG
    identity, and primitive trace.
38. Chaos shrink preserves hard-failure class and injection boundary.
39. Chaos corpus entries record coverage gained, hard-failure class,
    stream generator identity, and shrunk primitive trace when present.
40. Passing chaos mode cannot emit a full confidence packet.
41. Interaction fuzz profile schema exists and every run records profile
    hash.
42. Runner operations exist for run, replay, shrink, negative
    validation, and visual trace rendering.
43. Timeout and no-progress detection use declared profile fields and
    recorded progress marker history.
44. Chaos pressure coverage dimensions exist and are reported separately
    from semantic confidence coverage.
45. Chaos surface schema exists with deterministic construction,
    disabled-domain reporting, diagnostics policy, and Constitution
    boundary profile.
46. Visual diagnostics are emitted for failures when capture is
    available, or explicit unavailability is recorded.
47. Visual diagnostics are generated from traces, probes, progress
    markers, scene output, or Halcyon-rendered frames only.
48. Negative validation cases exist for semantic oracles,
    hard-failure detectors, replay validators, shrinker validators,
    reporter validators, and Constitution boundary checks.
49. A failed negative validation case invalidates any run relying on the
    failed oracle, detector, validator, or boundary check.
50. Constitution boundary profile and boundary check status are recorded
    in every packet and report.

---

## 48. Implementation Phases

### Phase 346.1: Spec and Contracts

* Add this SPEC.
* Identify first fixture and oracle set.
* Define Constitution boundary responsibilities for generators, probes,
  oracles, shrinker, replay, and reporter.
* No runtime changes required.

### Phase 346.2: In-Process Driver

* Add deterministic command driver.
* Drive through `RawInputEvent` and `InputFusion`.
* Support pointer lifecycle and frame/reconcile boundaries.
* Record primitive command traces.

### Phase 346.3: Intent Expansion

* Add intent command model.
* Expand pointer, keyboard, wheel, click, drag, and text intents into
  primitive command ranges.
* Add deterministic pointer path policies.
* Add deterministic pointer frame policies for coalesced and routed
  path samples.
* Mark threshold-crossing, hover-settle, press, drag, and release
  segments in expanded pointer paths.
* Add deterministic keyboard navigation policies.
* Record intent-to-primitive expansion metadata.

### Phase 346.4: Probe Registry

* Add runtime probes.
* Add ListView probe.
* Add active intent, primitive step, path policy, and incidental target
  probes.
* Add path exposure probes for coalesced samples, routed samples,
  threshold crossings, crossed targets, and capture transitions.
* Add authority provenance to every probe snapshot.
* Add TreeView probe only after ListView probe is stable.

### Phase 346.5: Oracle Registry

* Add universal oracles.
* Add oracle read-only enforcement and serialized derived-value records.
* Add ListView resize oracles.
* Add scroll and virtualization oracles.
* Add incidental navigation integrity oracle.

### Phase 346.6: Generator Registry

* Add resize generators.
* Add scroll + resize generator.
* Add keyboard/focus smoke generator.
* Add pointer path generators.
* Forbid endpoint teleportation in default user-like generators.
* Add keyboard navigation generators.
* Add incidental interaction generators.

### Phase 346.7: Shrink and Replay

* Add trace shrinker.
* Preserve injection boundary and authority mode during shrink.
* Add replay artifact.
* Add failure packet output.
* Support exact intent replay and exact primitive replay.
* Preserve primitive replay when shrinker reduces intent/path layers.

### Phase 346.8: CI Smoke

* Add bounded CI smoke profile.
* Emit confidence packet on pass.
* Fail on invariant violation.
* Record interaction fuzz profile hash.
* Record timeout policy and Constitution boundary status.

### Phase 346.9: Regression Promotion

* Add workflow to convert minimized failures into normal tests.
* Enforce SPEC-HAL-129 no-stub policy.

### Phase 346.10: Chaos / Bombardment Fuzzing

* Add chaos surface registry.
* Add seeded primitive chaos stream generator.
* Add continuous sampled pointer movement policies for chaos pressure.
* Add chaos pressure coverage dimensions.
* Add chaos hard-failure oracle set.
* Add panic, timeout, no-progress, non-finite sink, capture leak,
  dirty-settle, router contradiction, authority violation, replay
  nondeterminism, and unsupported-domain detectors.
* Add chaos failure packet fields.
* Add chaos primitive stream shrinker.
* Add chaos corpus integration.

### Phase 346.11: Chaos Developer Runner

* Add interaction fuzz profile schema.
* Add bounded developer command/profile for chaos runs.
* Add runner operations for run, replay, shrink, negative validation,
  and visual trace rendering.
* Add artifact directory layout.
* Add replay command output.
* Add chaos report output.
* Add visual trace output for pointer paths, command timeline, target
  timeline, progress markers, and failure frame context.
* Add declared timeout and no-progress detection.
* Add profile-hash enforcement for replay.

### Phase 346.12: Negative Validation and Constitution Gate

* Add deterministic negative validation cases for every required oracle,
  hard-failure detector, replay validator, shrinker validator, reporter
  validator, and Constitution boundary check.
* Add visual-oracle-substitution negative validation.
* Add authority-bypass negative validation.
* Add Constitution boundary profile schema.
* Add boundary check status to every packet and report.
* Reject runs that rely on failed negative validation.

---

## 49. First Target: ListView Column Resize

The first implementation target is ListView column resize because it
exercises multiple high-risk layers:

* separator hit-testing
* horizontal scroll projection
* pointer capture
* resize state machine
* column model mutation
* layout invalidation
* horizontal geometry re-resolution
* header/body/footer synchronization

The primary defect class is:

```text
first resize after setup or scroll changes width in the opposite direction,
while subsequent resizes behave correctly
```

This class must be covered by:

* ORACLE-346-R2
* ORACLE-346-R3
* ORACLE-346-R4
* ORACLE-346-R5
* ORACLE-346-R12

---

## 50. Confidence Semantics

The subsystem's passing output is meaningful only because:

* the inputs use the real runtime path
* the oracles are declared
* the probes read authoritative state
* the replay artifact is deterministic
* the coverage frontier is recorded
* skipped domains are named

A pass means:

```text
The declared interaction contracts survived the generated transaction
frontier for this fixture and seed set.
```

That is a strong smoke signal.

It is not a proof that no bug exists outside the frontier.

A passing chaos/bombardment run means only:

```text
No hard-failure oracle fired inside the executed chaos budget for the
declared surface and seed set.
```

That is a failure-discovery signal, not an interaction-confidence claim.

---

## 51. Closure Rule

This SPEC cannot be marked Implemented until failure artifacts,
confidence artifacts, chaos/bombardment failure-discovery artifacts,
negative validation artifacts, visual diagnostic artifacts, and
Constitution boundary check artifacts exist for the declared modes.

Finding bugs is insufficient.

Passing smoke with a bounded confidence packet is required for
contract/user-like fuzzing.

Replayable chaos failure packets and bounded chaos reports are required
for chaos/bombardment fuzzing.

Passing negative validation is required before any oracle, detector,
validator, or boundary check may support a passing report.

Visual diagnostics are required as trace-derived evidence when capture
is available, but they remain non-authoritative.
