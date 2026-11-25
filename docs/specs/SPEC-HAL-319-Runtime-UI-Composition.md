# SPEC-HAL-319: **Runtime UI Composition via Agent**

### *AI-Driven Wireframing and Layout Design Through the Agent Channel*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-317 (Agent Integration)
* SPEC-HAL-318 (Tower)
* SPEC-HAL-001 (Core Architecture)
* SPEC-HAL-002 (Constitution)

---

# 1. Purpose

This SPEC defines the capability for AI agents to compose, arrange, and iterate on
UI layouts at runtime — without writing Rust code, without compiling, without
leaving the running application.

The agent manipulates the widget tree directly through structured commands:
adding widgets, removing them, moving them, configuring their properties, and
arranging them into screens. The human watches the layout take shape in real-time.
When the design is approved, the agent captures a structural snapshot — the
**blueprint** — which becomes the specification for the production implementation.

This transforms Halcyon from a framework that agents can *test* into a framework
that agents can *design*.

---

# 2. Problem Statement

UI layout design is iterative. The traditional cycle is:

```
sketch → code → compile → run → look → adjust → recompile → ...
```

Each iteration takes minutes. For backend-focused developers who don't think in
visual terms naturally, the iteration is slower still — each cycle requires
translating spatial intuition into code.

The agent channel (SPEC-HAL-317) already gives agents read access (queries) and
input simulation (actions). What's missing is **structural write access** — the
ability to create, remove, move, and configure widgets in the live tree.

With structural write access, the cycle becomes:

```
human describes intent → agent composes widgets → layout renders immediately
→ human says "move that down" → agent adjusts → instant
→ human says "good, snapshot it" → agent captures blueprint
→ production implementation uses blueprint as the map
```

No compile cycle. No code translation. The prototype runs in the actual framework
with the actual theme, fonts, and layout engine. What you see IS what you ship.

---

# 3. Design Principles

### 3.1 The Widget Tree IS the Canvas

No separate "design mode" or "preview layer." The agent manipulates the same
WidgetTree that the engine uses for layout and rendering. The result is a real,
live, interactive UI — not a mockup.

### 3.2 Constitution Compliance

All tree manipulations go through the deferred mutation queue (SPEC-HAL-002
Phase 2). The agent cannot mutate the tree during layout or paint. The same
constitutional guarantees that protect the engine from user input protect it
from agent input.

### 3.3 Composition, Not Configuration

The agent composes UIs from Halcyon's existing widget library. It does not
create new widget types at runtime. The vocabulary is the widget catalog;
the agent arranges, nests, and configures — it does not invent.

### 3.4 Blueprint as Artifact

The output of a design session is a **blueprint** — a serialized description of
the widget tree structure, configuration, and layout. The blueprint is a document,
not code. It describes WHAT the UI looks like. The production implementation
translates the blueprint into proper Screen code with data bindings, validation,
and behavior.

### 3.5 Reversible

Every composition action can be undone. The agent can remove what it added, move
things back, clear the canvas. There is no destructive action that cannot be
reversed within the session.

---

# 4. Agent Composition Actions

New `AgentAction` variants for structural manipulation:

## 4.1 AddWidget

Create a widget and add it as a child of an existing node.

```json
{
    "AddWidget": {
        "parent_id": 2,
        "widget_type": "TextField",
        "index": 3,
        "config": {
            "placeholder": "Enter email",
            "hint": "User email address"
        }
    }
}
```

Response: `WidgetAdded { id: 15, parent_id: 2 }`

Supported widget types (initial set):
- Layout: `Flex`, `Container`, `Panel`, `GroupBox`, `SplitPane`, `ScrollArea`
- Input: `TextField`, `TextArea`, `ComboBox`, `Checkbox`, `Switch`, `Slider`,
  `SpinBox`, `RadioGroup`, `TagInput`, `DatePicker`, `TimePicker`, `ColorPicker`
- Display: `Label`, `ProgressBar`, `Gauge`, `LinearGauge`, `Sparkline`, `Divider`
- Navigation: `TabBar`, `Breadcrumbs`, `SegmentedControl`
- Data: `List`, `TreeView`, `PropertyEditor`
- Spacers: `FixedSpacer`, `FlexSpacer`

Config is type-specific and optional. Unknown fields are ignored.

## 4.2 RemoveWidget

Remove a widget and its entire subtree from the tree.

```json
{
    "RemoveWidget": {
        "widget_id": 15
    }
}
```

Response: `WidgetRemoved { id: 15, children_removed: 3 }`

## 4.3 MoveWidget

Reparent a widget under a different parent, at a specified index.

```json
{
    "MoveWidget": {
        "widget_id": 15,
        "new_parent_id": 8,
        "index": 0
    }
}
```

Response: `WidgetMoved { id: 15, old_parent: 2, new_parent: 8 }`

## 4.4 ConfigureWidget

Update configuration on an existing widget.

```json
{
    "ConfigureWidget": {
        "widget_id": 15,
        "config": {
            "placeholder": "Updated placeholder",
            "hint": "Updated hint"
        }
    }
}
```

Response: `WidgetConfigured { id: 15 }`

Supported config keys (per widget type):
- All: `hint`
- Label: `text`
- TextField: `placeholder`
- Container: `width`, `height`, `background`
- Flex: `direction` ("row" | "column"), `spacing`, `padding`
- GroupBox: `title`
- FixedSpacer: `width`, `height`
- ProgressBar: `value` (0.0-1.0)
- ComboBox: `options` (array of strings), `selected_index`
- Checkbox: `label`, `checked`
- Slider: `min`, `max`, `value`

## 4.5 SetLayout

Set flex layout parameters on a widget.

```json
{
    "SetLayout": {
        "widget_id": 15,
        "flex_grow": 1.0,
        "flex_shrink": 1.0,
        "min_width": 200.0,
        "max_width": 400.0,
        "min_height": 32.0
    }
}
```

All fields optional. Only provided fields are updated.

## 4.6 ClearChildren

Remove all children of a node (reset a container).

```json
{
    "ClearChildren": {
        "widget_id": 2
    }
}
```

Response: `ChildrenCleared { id: 2, removed: 5 }`

## 4.7 Snapshot

Capture the current widget tree as a blueprint document.

```json
{
    "Snapshot": {
        "name": "settings_screen_v1"
    }
}
```

Response: `SnapshotCaptured { name: "settings_screen_v1", widget_count: 24 }`

The blueprint is emitted as a `QueryResult` event containing the full tree
structure with widget types, configurations, layout parameters, and hints.

---

# 5. Blueprint Format

A blueprint is a JSON document describing the widget tree:

```json
{
    "name": "settings_screen_v1",
    "captured_at": "2026-05-06T15:30:00Z",
    "root": {
        "type": "Flex",
        "config": { "direction": "column", "spacing": 8.0 },
        "children": [
            {
                "type": "Label",
                "config": { "text": "Settings", "hint": "Screen title" }
            },
            {
                "type": "GroupBox",
                "config": { "title": "General" },
                "children": [
                    {
                        "type": "TextField",
                        "config": {
                            "placeholder": "Display name",
                            "hint": "User's display name. Required."
                        },
                        "layout": { "flex_grow": 1.0 }
                    },
                    {
                        "type": "ComboBox",
                        "config": {
                            "hint": "Language selection",
                            "options": ["English", "Russian", "Latvian"]
                        },
                        "layout": { "flex_grow": 1.0 }
                    }
                ]
            },
            {
                "type": "GroupBox",
                "config": { "title": "Network" },
                "children": [
                    {
                        "type": "TextField",
                        "config": {
                            "placeholder": "Server address",
                            "hint": "Remote server hostname or IP"
                        }
                    },
                    {
                        "type": "SpinBox",
                        "config": {
                            "hint": "Server port. Range 1-65535."
                        }
                    },
                    {
                        "type": "Checkbox",
                        "config": {
                            "label": "Use TLS",
                            "hint": "Enable TLS encryption for server connection"
                        }
                    }
                ]
            },
            {
                "type": "Flex",
                "config": { "direction": "row", "spacing": 8.0 },
                "children": [
                    { "type": "FlexSpacer" },
                    {
                        "type": "Button",
                        "config": { "text": "Cancel", "hint": "Discard changes" }
                    },
                    {
                        "type": "Button",
                        "config": { "text": "Save", "hint": "Apply and save settings" }
                    }
                ]
            }
        ]
    }
}
```

The blueprint contains:
- Widget types and configurations (WHAT)
- Layout parameters (HOW it's arranged)
- Semantic hints (WHY each widget exists)
- Tree structure (hierarchy and ordering)

The blueprint does NOT contain:
- Widget IDs (runtime-assigned)
- Pixel positions (computed by layout engine)
- Data bindings (production concern)
- Validation rules (production concern)
- Event handlers (production concern)
- Theme values (resolved at runtime)

---

# 6. Canvas App

A minimal Halcyon application that serves as the design surface:

```rust
// examples/canvas.rs
// A blank canvas for agent-driven UI composition.
// Start this, then connect via Tower or direct WebSocket.
App::new(
    AppConfig::new("Canvas")
        .with_size(1200.0, 800.0)
        .with_agent_server(9317),
).run(CanvasScreen::new());
```

`CanvasScreen` starts with a single root Flex column and nothing else. The
agent builds everything on top.

---

# 7. Conversation-Driven Design

The typical workflow:

### 7.1 Human Describes Intent

"I need a settings screen. Three sections: General, Network, Advanced. General
has display name and language selection. Network has server address, port, and
a TLS checkbox. Advanced has log level and a clear cache button. Save and Cancel
buttons at the bottom."

### 7.2 Agent Composes

The application author or an authorized agent sends composition actions:
1. AddWidget: Flex column (root layout)
2. AddWidget: Label "Settings" (title)
3. AddWidget: GroupBox "General"
4. AddWidget: TextField with hint "Display name"
5. AddWidget: ComboBox with hint "Language" + options
6. AddWidget: GroupBox "Network"
7. ... and so on

Each widget appears immediately in the running Canvas app.

### 7.3 Human Reviews and Adjusts

"The Network section should come before Advanced. Add more spacing between
sections. The save button should be primary-styled."

Agent sends MoveWidget, ConfigureWidget, SetLayout commands.

### 7.4 Snapshot

"Good. Snapshot it."

Agent sends Snapshot command. Blueprint saved as JSON.

### 7.5 Production Implementation

The application author turns the approved blueprint into a Screen implementation
with data bindings, validation and event handling. The layout matches the approved
design because it was composed in the actual framework.

---

# 8. Invariants

### INV-319-1: Constitution Compliance

All composition actions are processed during Phase 2 (Deferred Mutations).
No tree mutation occurs during layout or paint.

### INV-319-2: Existing Widgets Only

The agent cannot define new widget types at runtime. `AddWidget` only
creates instances of widgets in Halcyon's compiled widget catalog.

### INV-319-3: Blueprint Is Data, Not Code

Blueprints contain no executable code, no closures, no function references.
They are pure structural descriptions serializable as JSON.

### INV-319-4: Reversibility

Every composition action has a reverse action. AddWidget can be reversed by
RemoveWidget. MoveWidget can be reversed by MoveWidget back. The agent can
always return to any previous state within a session.

### INV-319-5: Agent Channel Only

Composition actions arrive exclusively through the agent channel
(WebSocket or embedded). No other entry point. FORBID-318-1 applies.

### INV-319-6: No Data Binding at Design Time

Blueprints describe structure, not behavior. Data sources, validation rules,
event handlers, and state management are production concerns added during
implementation — never during design-time composition.

---

# 9. Forbidden Behaviors

### FORBID-319-1: No Runtime Widget Type Creation

The agent MUST NOT create new widget types, define new structs, compile code,
or execute arbitrary Rust at runtime. Composition is arrangement of existing
pieces, not invention of new ones.

### FORBID-319-2: No Direct Tree Mutation

Composition actions MUST go through the agent action channel and be processed
during Phase 2. The agent MUST NOT bypass the mutation queue.

### FORBID-319-3: No Style Hardcoding in Blueprints

Blueprints MUST NOT contain hardcoded colors, font sizes, or pixel values that
should come from the theme. Layout parameters (spacing, min/max sizes) are
acceptable. Presentation values (colors, fonts) are not.

---

# 10. Implementation Phases

### Phase 1 — Core Composition Actions

AddWidget, RemoveWidget, ConfigureWidget for 10 key widget types.
Canvas app. Basic blueprint snapshot.

### Phase 2 — Layout Control

MoveWidget, SetLayout, ClearChildren.
Full flex parameter support. Widget reordering.

### Phase 3 — Blueprint System

Structured blueprint export. Blueprint-to-code generation hints.
Blueprint diffing (compare two snapshots).

### Phase 4 — Conversation Integration

Tower-mediated composition: human describes in natural language,
Tower's LLM translates to composition actions. Interactive
refinement loop.

### Phase 5 — Screen Templates

Pre-built composition templates (form screen, settings screen,
dashboard layout, list-detail). Agent starts from a template
and customizes.

---

# 11. Relationship to Accessibility

The semantic hints added during composition serve both:
- AI agents navigating the UI (SPEC-HAL-317)
- Screen readers for visually impaired users (future accessibility SPEC)

The blueprint's hint annotations become the foundation for an accessibility
label system. Composition-time is the natural moment to describe what
widgets are for — the designer (human or AI) is thinking about purpose,
not just position.

---

# 12. Relationship to Testing

Blueprints composed during design can be instantiated as test fixtures.
Tower can load a blueprint, instantiate the tree, and run automated
exploration — verifying keyboard navigation, focus order, and widget
reachability without writing test-specific setup code.

---

# 13. Future Extensions

- **Multi-screen blueprints:** compose navigation flows (screen A → screen B)
- **Responsive variants:** blueprint variants for different window sizes
- **Animation annotations:** mark widgets for entrance/exit transitions
- **Data shape declarations:** declare the shape of data each widget will bind to
  (enables type-checked data binding generation)
- **Collaborative composition:** multiple agents or human+agent composing together
  via the same agent channel

---

# 14. Conclusion

SPEC-HAL-319 transforms Halcyon from a framework agents can observe and test
into a framework agents can design. The composition vocabulary is small
(7 actions), the constitutional compliance is guaranteed (Phase 2 mutations),
and the output (blueprints) bridges the gap between design intent and
production implementation.

The prototype IS the target medium. What you see is what you ship.
