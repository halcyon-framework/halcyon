# SPEC-HAL-317: **Agent Integration Architecture**

### *Layered Agent Access, Event Observation, and Autonomous Testing*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-001 (Core Architecture)
* SPEC-HAL-002 (Constitution)
* SPEC-HAL-316 (Annotation Layer)

---

# 1. Purpose

This SPEC defines how external agents (LLM-based, scripted, or
autonomous) interact with Halcyon applications. The architecture
enables agents to observe, navigate, test, and annotate — but
never bypass the UI's own validation and mutation paths.

Halcyon apps become agent-accessible by default: any Halcyon
application exposes a structured channel for agent interaction
without per-app custom integration.

---

# 2. Core Principles

**CP-317-1: Agents interact THROUGH the UI, not around it.**
An agent triggers the same events a user would. All validation,
state transitions, and business logic fire exactly as they would
for a human. The agent is a synthetic user with deeper read access,
not a backdoor.

**CP-317-2: Read-deep, write-shallow.**
Agents can read deeply into widget state, data layers, and
structural metadata. But write operations go through Layer 1
(surface actions) — the same input events a user produces.
No direct data mutation.

**CP-317-3: The widget tree IS the world model.**
Halcyon's WidgetTree with stable IDs, deterministic rebuild,
and phase-separated state is already a queryable semantic graph.
Agents don't need a separate scene representation — the tree
is the source of truth.

**CP-317-4: Agents observe via event channel, act via action channel.**
Two independent channels: one for observation (what happened),
one for action (what to do). The agent never needs to poll —
the event channel pushes.

**CP-317-5: Visual output through the Annotation Layer.**
When an agent needs to show something to the user — a finding,
a suggestion, a test result — it uses SPEC-HAL-316 annotations:
highlights, connectors, labels, circles. No custom rendering.

---

# 3. Three-Layer Access Model

## 3.1 Layer 1 — Surface (User-Level)

The agent interacts with widgets exactly as a human would.

**Capabilities:**
- Trigger pointer events (click, double-click, drag)
- Trigger keyboard events (key press, text input)
- Trigger focus changes (tab, direct focus)
- Activate actions (buttons, menu items)
- Navigate (screen push/pop, scroll)

**Mechanism:**
Synthetic `FusedInputEvent` injected into the event pipeline
at the same point OS events enter. The event flows through
`dispatch_engine_action` → `route_input_event` →
`dispatch_to_widget` — identical path to real input.

```rust
pub enum AgentAction {
    Click { widget_id: WidgetId },
    DoubleClick { widget_id: WidgetId },
    TypeText { widget_id: WidgetId, text: String },
    KeyPress { widget_id: WidgetId, key: KeyCode, modifiers: Modifiers },
    Focus { widget_id: WidgetId },
    Scroll { widget_id: WidgetId, delta: f32 },
    Activate { widget_id: WidgetId },
}
```

**Firewall:** All actions pass through the same validation
the UI applies. A disabled button rejects agent clicks.
A read-only field rejects agent text input. No bypass.

## 3.2 Layer 2 — Functional (Widget API)

The agent accesses widget-specific read APIs and safe operations
that a power user would have access to through the UI but
faster and programmatically.

**Capabilities:**
- Query widget state (selected index, text content, expanded nodes)
- Invoke widget operations (filter, search, sort, clear filter)
- Read option lists (ComboBox items, ListView rows, TreeView nodes)
- Query validation state (current errors, valid/invalid)
- Read layout geometry (bounds, visibility, clip state)

**Mechanism:**
Widgets implement the `AgentInspectable` trait:

```rust
pub trait AgentInspectable {
    /// Widget type name for agent identification.
    fn agent_type(&self) -> &'static str;

    /// Structured snapshot of widget state.
    fn agent_state(&self) -> AgentState;

    /// Available functional operations.
    fn agent_capabilities(&self) -> Vec<AgentCapability>;

    /// Execute a functional operation.
    fn agent_invoke(&mut self, op: &str, args: &AgentArgs) -> AgentResult;
}
```

**Example — ComboBox:**
```rust
impl AgentInspectable for ComboBox {
    fn agent_type(&self) -> &'static str { "ComboBox" }

    fn agent_state(&self) -> AgentState {
        AgentState::map([
            ("selected_index", self.selected_index().into()),
            ("selected_text", self.selected_text().into()),
            ("option_count", self.options().len().into()),
            ("popup_open", self.popup_open.into()),
            ("focused", self.focused.into()),
        ])
    }

    fn agent_capabilities(&self) -> Vec<AgentCapability> {
        vec![
            AgentCapability::read("options", "List all available options"),
            AgentCapability::read("filter", "Current filter text"),
            AgentCapability::action("set_filter", "Apply a filter string"),
            AgentCapability::action("clear_filter", "Clear the current filter"),
        ]
    }

    fn agent_invoke(&mut self, op: &str, args: &AgentArgs) -> AgentResult {
        match op {
            "options" => AgentResult::List(self.option_texts()),
            "filter" => AgentResult::Text(self.filter_text().into()),
            "set_filter" => {
                let text = args.get_str("text")?;
                self.set_filter(text);
                AgentResult::Ok
            }
            "clear_filter" => {
                self.clear_filter();
                AgentResult::Ok
            }
            _ => AgentResult::UnknownOperation,
        }
    }
}
```

**Firewall:** Layer 2 operations that mutate visible state
(like `set_filter`) trigger the same invalidation and rebuild
paths as user interaction. No silent state changes.

## 3.3 Layer 3 — Deep (Structural / Schema)

The agent reads the structural metadata and relationships
that aren't visible in the UI surface but govern behavior.

**Capabilities:**
- Query the widget tree structure (parent/child, sibling order)
- Read widget policies (focusable, capture-eligible, hit-testable)
- Query data bindings (what data source feeds this widget)
- Read constraint relationships (which widgets affect which)
- Query navigation structure (screen stack, available routes)
- Read accessibility metadata (labels, roles, descriptions)

**Mechanism:**
The `AgentTreeQuery` API operates on the WidgetTree directly:

```rust
pub struct AgentTreeQuery<'a> {
    tree: &'a WidgetTree,
}

impl<'a> AgentTreeQuery<'a> {
    /// All visible, focusable widgets with their types and bounds.
    pub fn focusable_widgets(&self) -> Vec<AgentWidgetInfo>;

    /// Widget tree path from root to target.
    pub fn ancestry(&self, widget_id: WidgetId) -> Vec<WidgetId>;

    /// Siblings of a widget in tree order.
    pub fn siblings(&self, widget_id: WidgetId) -> Vec<WidgetId>;

    /// Children of a widget.
    pub fn children(&self, widget_id: WidgetId) -> Vec<WidgetId>;

    /// Widget info: type, bounds, policy, focus state, visibility.
    pub fn inspect(&self, widget_id: WidgetId) -> Option<AgentWidgetInfo>;

    /// All widgets of a specific type.
    pub fn widgets_of_type(&self, type_name: &str) -> Vec<WidgetId>;

    /// Tab order (sequential focus traversal).
    pub fn tab_order(&self) -> Vec<WidgetId>;
}
```

**Firewall:** Layer 3 is strictly read-only. No mutations.
No access to internal RefCell state. No access to raw
memory or pointers. The query operates on the public
tree structure only.

---

# 4. Event Channel (Observation)

## 4.1 AgentEventChannel

A broadcast channel that streams UI events to observing agents.
The agent receives a structured feed of everything that happens
in the application without polling.

```rust
pub struct AgentEvent {
    pub timestamp: Instant,
    pub kind: AgentEventKind,
}

pub enum AgentEventKind {
    // Widget lifecycle
    WidgetCreated { widget_id: WidgetId, widget_type: String },
    WidgetDestroyed { widget_id: WidgetId },

    // Focus
    FocusChanged { old: Option<WidgetId>, new: Option<WidgetId> },
    CaptureEntered { widget_id: WidgetId },
    CaptureLeft { widget_id: WidgetId },

    // Input (what the user did)
    PointerDown { widget_id: WidgetId, button: PointerButton, pos: (f32, f32) },
    PointerUp { widget_id: WidgetId, button: PointerButton },
    KeyDown { widget_id: WidgetId, key: KeyCode, modifiers: Modifiers },
    TextInput { widget_id: WidgetId, text: String },

    // State changes
    ValueChanged { widget_id: WidgetId, old: String, new: String },
    SelectionChanged { widget_id: WidgetId, selected: Vec<usize> },
    ValidationError { widget_id: WidgetId, error: String },
    ValidationCleared { widget_id: WidgetId },

    // Navigation
    ScreenPushed { screen_id: ScreenId },
    ScreenPopped,

    // Layout
    LayoutCompleted { duration_us: u64 },

    // Application
    FrameRendered { frame_number: u64, duration_us: u64 },
    Error { source: String, message: String },

    // Custom (consumer-defined)
    Custom { tag: String, payload: String },
}
```

## 4.2 Subscription

```rust
impl App {
    /// Subscribe to the agent event channel.
    /// Returns a receiver that gets all UI events.
    pub fn agent_event_channel(&self) -> crossbeam_channel::Receiver<AgentEvent>;
}
```

Events are fire-and-forget from the app's perspective.
If no agent is listening, events are dropped (no backpressure).
The channel is bounded (default 10,000 events) to prevent
unbounded memory growth from unread events.

## 4.3 Event Filtering

Agents can filter at the receiver side. No filtering at the
source — all events are always emitted. This ensures agents
can't miss events due to misconfigured filters.

---

# 5. Action Channel (Agent → App)

```rust
impl App {
    /// Get the agent action sender.
    /// Actions are processed at the start of the next frame,
    /// in the same phase as OS input events.
    pub fn agent_action_sender(&self) -> crossbeam_channel::Sender<AgentAction>;
}
```

Actions are queued and processed in Phase 1 (Event Intake)
alongside OS events. They enter the same dispatch pipeline.
This guarantees:
- Actions respect widget state (disabled, hidden, etc.)
- Actions trigger the same side effects as user input
- Actions are ordered with respect to real user input
- No race conditions between agent and user actions

## 5.1 External Server Capability Profiles

The localhost WebSocket transport has two explicit profiles:

- `Full` preserves Wireframe/Tower composition authority from
  SPEC-HAL-319/320. It is selected by `AppConfig::with_agent_server`
  and is not the consumer-application verification boundary.
- `DevelopmentUiControl` is selected only by
  `AppConfig::with_development_agent_server`. It is available only in
  debug builds and admits `Click`, `DoubleClick`, `Focus`, `Scroll`,
  `KeyPress`, `TypeText`, `FocusState`, `WidgetState`, and filtered,
  bounded `FindWidgets`.

`DevelopmentUiControl` rejects structural composition, widget
configuration, annotation, snapshot, window mutation/introspection,
text measurement, full-tree/list queries, tab-order dumps, and
`Sequence`. The profile is enforced before the action enters the
engine channel. `Sequence` is excluded because the engine does not
execute it atomically; Tower owns expansion under the `Full` profile.

Targeted discovery defaults to 32 results and cannot exceed 64.
`FindWidgets` requires a type or label filter. A `WidgetState` record
returns at most 64 child IDs and explicitly reports total count and
truncation. Text and hint fields are bounded. These limits constrain
external context without weakening the read-only nature of queries.

---

# 6. Use Cases

## 6.1 Automated Accessibility Testing

Agent walks the tab order, verifies every focusable widget
is reachable, verifies labels exist, verifies keyboard-only
operation is possible.

```
Agent:
1. Query tab_order() → [w1, w2, w3, ...]
2. For each widget: Focus(w_n), verify focus event received
3. For each interactive widget: Activate(w_n), verify response
4. Report: "Widget w7 (ComboBox) is not reachable via Tab"
5. Annotate: Highlight w7 with error style via SPEC-HAL-316
```

## 6.2 Keyboard-Only Operation Audit

Agent attempts to complete a workflow using only keyboard
actions. Reports any step that requires a pointer.

```
Agent:
1. Start recording event channel
2. Execute workflow: Tab → Enter → type text → Tab → Enter
3. Monitor: did the workflow complete?
4. If stuck: "Step 3 requires mouse click on widget w12"
5. Annotate: Draw Bezier path showing the ideal keyboard route
```

## 6.3 Ergonomic Path Analysis

Agent measures click/keystroke counts for common workflows.
Proposes optimizations.

```
Agent:
1. Observe user performing task (event channel)
2. Count: 14 clicks, 3 screen transitions, 22 keystrokes
3. Analyze: "Steps 5-8 could be replaced with Ctrl+Shift+S"
4. Annotate: Temporal link showing the current path (faded)
   and the proposed shortcut (bold)
```

## 6.4 Continuous Testing Agent

Runs alongside development. Developer implements a feature,
agent continuously tests it.

```
Agent:
1. Monitor event channel for WidgetCreated events
2. New widget detected → inspect capabilities
3. Exercise all Layer 1 actions (click, type, focus)
4. Verify no ValidationError events without recovery
5. Verify no Error events
6. Report findings via annotation labels on the affected widgets
```

## 6.5 Fuzzy/Chaos Testing

Agent generates random but structurally valid action sequences.

```
Agent:
1. Query all interactive widgets (Layer 3)
2. Generate random action sequence:
   Click(w3) → TypeText(w7, "asdf") → KeyPress(w2, Escape)
3. Monitor event channel for crashes, panics, error events
4. If crash: record action sequence as reproduction steps
5. Annotate: highlight the crash path with styled connectors
```

## 6.6 Live Error Reporting

Agent monitors the event channel for errors and validation
failures, creates persistent annotation overlays.

```
Agent:
1. Subscribe to event channel, filter: ValidationError, Error
2. On error: create AnnotationLabel at the source widget
3. On error cleared: remove the annotation
4. Persistent overlay of all current issues
```

---

# 7. Framework Integration

## 7.1 App-Level Setup

```rust
let app = App::new(config);

// Enable agent channels
let (event_rx, action_tx) = app.enable_agent_channels();

// Spawn agent on separate thread
std::thread::spawn(move || {
    let agent = MyTestingAgent::new(event_rx, action_tx);
    agent.run();
});

app.run(MainScreen::new());
```

For explicit consumer-app development verification:

```rust
let config = AppConfig::new("Development Workstation")
    .with_development_agent_server(9317);
App::new(config).run(MainScreen::new());
```

The server continues to bind only `127.0.0.1`. The typed
`core::agent_client::AgentClient` performs targeted
`FindWidgets → WidgetState → input` workflows. The `agent_control`
binary exposes the same bounded operations without full
`WidgetTree`, `VisibleWidgets`, or `Snapshot` output.

## 7.2 Phase Integration

Agent actions are drained at the start of Phase 1
(Event Intake), interleaved with OS events:

```
Phase 1: OS Event Intake
  1a. Drain OS events from InputFusion
  1b. Drain agent actions from action channel
  1c. Convert agent actions to FusedInputEvents
  1d. Process all events through dispatch_engine_action
Phase 2: Deferred Mutations
Phase 3: Rebuild
...
```

Agent events are emitted throughout all phases:
- Focus events during Phase 1 (dispatch)
- ValueChanged during Phase 2 (mutations)
- LayoutCompleted after Phase 4
- FrameRendered after Phase 6

## 7.3 Widget Opt-In

`AgentInspectable` is an optional trait. Widgets that don't
implement it are still accessible via Layer 1 (surface actions)
and Layer 3 (tree queries). Layer 2 (functional API) requires
the trait implementation.

Default implementation provided for common patterns:

```rust
/// Blanket Layer 2 for all widgets — basic state.
impl<W: Widget> AgentInspectable for W {
    default fn agent_type(&self) -> &'static str {
        self.debug_type_name()
    }

    default fn agent_state(&self) -> AgentState {
        AgentState::map([
            ("type", self.debug_type_name().into()),
            ("focusable", self.focusable().into()),
            ("hit_testable", self.hit_testable().into()),
        ])
    }

    default fn agent_capabilities(&self) -> Vec<AgentCapability> {
        Vec::new()
    }

    default fn agent_invoke(&mut self, _op: &str, _args: &AgentArgs) -> AgentResult {
        AgentResult::UnknownOperation
    }
}
```

Widgets override with specific capabilities as needed.

---

# 8. Consumer Event Layer

Consumers (application developers) can emit custom events
into the agent event channel for domain-specific observation.

```rust
cx.emit_agent_event(AgentEventKind::Custom {
    tag: "trade_executed".into(),
    payload: serde_json::to_string(&trade).unwrap(),
});
```

This allows agents to observe application-level events
(trades, alerts, state transitions) alongside UI events,
building a complete picture of what happened and why.

---

# 9. Invariants

**INV-317-1:** Agent actions enter the same dispatch pipeline
as OS events. No bypass. No shortcut. Same validation, same
side effects, same ordering guarantees.

**INV-317-2:** Layer 3 (tree queries) is strictly read-only.
No mutations to the WidgetTree, AppState, or widget internal
state from Layer 3 queries.

**INV-317-3:** Agent event channel is fire-and-forget. If no
agent is listening, events are dropped. The application never
blocks waiting for an agent to consume events.

**INV-317-4:** Agent actions are processed during Phase 1
(Event Intake). They never execute during layout, paint,
or rebuild phases. Constitutional compliance (SPEC-HAL-002
Pillar B — Phase Separation).

**INV-317-5:** Widget functional operations (Layer 2) trigger
the same invalidation and rebuild paths as user interaction.
No silent state changes.

**INV-317-6:** The agent event channel is bounded. Default
capacity 10,000 events. Overflow drops oldest events (not
newest). The application never blocks on a full channel.

**INV-317-7:** Every external Agent server action is authorized
against its configured capability profile before entering the
bounded engine action channel.

**INV-317-8:** `DevelopmentUiControl` is localhost-only,
debug-build-only, and write-shallow. Its admitted writes are ordinary
surface input routed through the canonical fused-input path.

**INV-317-9:** Development inspection is targeted and
context-bounded. It cannot request a full widget-tree, visible-widget,
tab-order, or snapshot dump through this profile.

---

# 10. Forbidden Behaviors

**FORBID-317-1:** Agents MUST NOT mutate data by circumventing
UI logic. All write operations go through Layer 1 (surface
actions) which pass through the same validation and business
logic as user input. Direct data mutation is the footgun.

**FORBID-317-2:** Agents MUST NOT hold persistent references
to widget instances. All interaction is via WidgetId. Agents
observe snapshots, not live references. Constitutional
compliance (SPEC-HAL-002 Pillar C).

**FORBID-317-3:** Agent event emission MUST NOT block the
render loop. Events are sent via try_send (non-blocking).
If the channel is full, events are dropped, not queued
with backpressure.

**FORBID-317-4:** Layer 2 `agent_invoke` MUST NOT perform
operations that would be impossible through the UI. If a
user can't do it by clicking, the agent can't do it through
Layer 2. Layer 2 is convenience, not privilege escalation.

**FORBID-317-5:** A production-shaped consumer MUST NOT expose
SPEC-HAL-319 composition or window mutation through the development
UI-control profile. Rejected actions MUST NOT be queued for Phase 1
or Phase 2 processing.

**FORBID-317-6:** The development profile MUST NOT be enabled in
release builds and MUST NOT bind a non-loopback interface.

---

# 11. Relationship to SPEC-HAL-316 (Annotation Layer)

The annotation layer is the agent's visual output channel.
When an agent discovers something, it expresses it through
annotations:

| Agent Finding | Annotation Output |
|--------------|-------------------|
| Widget unreachable via keyboard | AnnotationHighlight (error) + AnnotationLabel |
| Ergonomic shortcut available | AnnotationConnector (Bezier) between start and target |
| Validation error detected | AnnotationLabel with error style |
| Test failure | AnnotationHighlight (spotlight) on failed widget |
| Workflow path | AnnotationConnector chain with temporal decay |

The agent uses `AnnotationContext` (SPEC-HAL-316 §4) to
manage its visual output. Annotations are tagged with the
agent's ID for batch management (show/hide agent overlays).

---

# 12. Implementation Phases

**Phase 1:** Event channel — AgentEvent enum, broadcast
sender on App, bounded crossbeam channel. Emit focus,
input, and value-change events from the existing dispatch
pipeline.

**Phase 2:** Action channel — AgentAction enum, sender
on App, drain during Phase 1. Synthetic FusedInputEvent
generation from AgentActions.

**Phase 3:** Layer 3 queries — AgentTreeQuery over
WidgetTree. tab_order, focusable_widgets, inspect, ancestry.

**Phase 4:** Layer 2 trait — AgentInspectable with default
impl. Per-widget overrides for TextField, ComboBox,
ListView, TreeView, PropertyEditor.

**Phase 5:** Integration with SPEC-HAL-316 — agent
annotation output via AnnotationContext.

**Phase 6:** External capability profiles — ingress authorization,
bounded targeted queries, explicit development-only consumer control,
and typed client tooling.

---

# 13. Acceptance Criteria

1. Agent event channel receives focus change events
2. Agent event channel receives value change events
3. Agent action channel triggers Click → widget receives pointer event
4. Agent action channel triggers TypeText → widget receives text input
5. Disabled widget rejects agent Click (same as user click)
6. Layer 3 query returns correct tab order
7. Layer 2 ComboBox: agent reads options, applies filter
8. Agent annotations render via SPEC-HAL-316
9. Event channel drops events when full (no blocking)
10. Agent action during layout phase is deferred to next Phase 1
11. Multiple agents can observe simultaneously (broadcast)
12. Custom consumer events appear in agent event channel
13. Agent stress test: 1000 random actions produce no panic
14. Development profile admits ordinary fused input and bounded targeted queries
15. Development profile rejects every composition/window action before enqueue
16. Filtered discovery and widget-state children enforce documented bounds
17. Full profile retains SPEC-HAL-319 Wireframe/Tower composition authority
18. Click, DoubleClick, and KeyPress produce complete down/up fused-input gestures
