# SPEC-HAL-321: Assembly Composition Protocol

### *Compound Composition Actions for Complex Layouts, Navigation, and Multi-View Wireframing*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-317 (Agent Integration Architecture)
* SPEC-HAL-319 (Runtime UI Composition)
* SPEC-HAL-228 (NavigationHost)
* SPEC-HAL-082 (Multi-Window Architecture)
* SPEC-HAL-081B (Multi-Surface Compositor)

---

# 1. Problem

The current composition protocol (SPEC-HAL-319) operates at the widget level: AddWidget, RemoveWidget, ConfigureWidget, MoveWidget. These are sufficient for flat forms and simple layouts but cannot express:

- Tabbed interfaces (TabBar + N switchable content regions)
- Wizard/stepped flows (Breadcrumbs + N sequential steps + navigation controls)
- Split layouts (SplitPane with named leading/trailing regions)
- Multi-window applications (multiple OS windows, each with its own widget tree)
- Navigation flows (Screen A → Screen B with back/forward)

Building these from widget-level primitives requires 15-30 coordinated steps with correct ID sequencing, widget wiring, and structural relationships. An LLM cannot reliably produce these sequences. A human using the MCP Bridge tools would need to make dozens of tool calls with manual ID tracking.

# 2. Solution

An **assembly layer** above the widget layer. Assembly actions are compound operations that the engine decomposes into widget-level operations internally, returning structured results with named IDs.

Two modes:

**Wireframe mode (primary, immediate):** All views are simultaneously visible. Tabbed interfaces show all tab contents stacked/labeled. Wizards show all steps. Multi-window apps show all windows as labeled regions. No runtime navigation wiring — the structure is a visual blueprint for human implementation. Annotations capture navigation intent.

**Live mode (future):** Assembly actions wire up real NavigationHost + ScreenHost + TabBar sinks for working navigation at runtime. Not in scope for this SPEC — requires callback wiring that the composition protocol doesn't support yet.

# 3. Assembly Actions

New variants added to `AgentAction` (src/core/agent.rs).

## 3.1 CreateTabLayout

Create a tabbed interface with N content regions.

```rust
AgentAction::CreateTabLayout {
    parent_id: u64,
    tabs: Vec<String>,  // Tab labels
    index: Option<usize>,
}
```

**Engine decomposition (wireframe mode):**

1. Create a Flex column container (the assembly root).
2. Create a TabBar widget with N tabs. Visual only — no sink wiring.
3. For each tab, create a Flex column content container with:
    - A SectionHeader: "Tab: {label}"
    - Agent hint: "Content region for tab '{label}'"
4. Mount all content containers as children of the assembly root, below the TabBar.
5. Mount assembly root under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateTabLayout",
    assembly_id: u64,       // Root container widget ID
    tab_bar_id: u64,        // TabBar widget ID
    content_ids: Vec<u64>,  // One per tab, in order
}
```

## 3.2 CreateWizard

Create a stepped wizard flow with N steps.

```rust
AgentAction::CreateWizard {
    parent_id: u64,
    steps: Vec<String>,  // Step labels
    index: Option<usize>,
}
```

**Engine decomposition (wireframe mode):**

1. Create a Flex column container (assembly root).
2. Create Breadcrumbs widget with N crumbs. Visual only.
3. Create a Divider below the breadcrumbs.
4. For each step, create a Flex column content container with:
    - A SectionHeader: "Step {n}: {label}"
    - A Divider at the bottom
    - Agent hint: "Content region for step {n}: '{label}'"
5. Create a Flex row at the bottom with Back + Next buttons (visual only).
6. Mount everything under assembly root, under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateWizard",
    assembly_id: u64,
    breadcrumb_id: u64,
    step_ids: Vec<u64>,     // One per step, in order
    back_button_id: u64,
    next_button_id: u64,
}
```

## 3.3 CreateSplitLayout

Create a resizable two-region layout.

```rust
AgentAction::CreateSplitLayout {
    parent_id: u64,
    orientation: String,      // "horizontal" or "vertical"
    ratio: Option<f32>,       // Initial split ratio [0.0, 1.0]. Default: 0.5
    index: Option<usize>,
}
```

**Engine decomposition:**

1. Create a SplitPane with orientation and ratio.
2. Create two Flex column children (leading and trailing).
3. Set agent hints: "Leading panel" and "Trailing panel".
4. Mount under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateSplitLayout",
    assembly_id: u64,       // SplitPane widget ID
    leading_id: u64,        // Leading panel content container
    trailing_id: u64,       // Trailing panel content container
}
```

## 3.4 CreateWindowRegion

Represent a separate window as a named, visually distinct region on the wireframe canvas. In wireframe mode, this is a GroupBox with a distinctive title — NOT a real OS window. The blueprint annotation captures that it should become a real window in implementation.

```rust
AgentAction::CreateWindowRegion {
    parent_id: u64,
    title: String,
    width: Option<f32>,
    height: Option<f32>,
    index: Option<usize>,
}
```

**Engine decomposition:**

1. Create a GroupBox with title "[Window] {title}".
2. Create a Flex column content container inside.
3. Set agent hint: "Window region: '{title}'. Implement as separate OS window."
4. If width/height provided, wrap in a Container with fixed dimensions.
5. Mount under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateWindowRegion",
    assembly_id: u64,       // GroupBox widget ID
    content_id: u64,        // Content container for this window's widgets
}
```

## 3.5 CreateFormSection

Create a labeled form section with title and content area. Common pattern — avoids the repetitive GroupBox + Flex + SectionHeader sequence.

```rust
AgentAction::CreateFormSection {
    parent_id: u64,
    title: String,
    description: Option<String>,
    index: Option<usize>,
}
```

**Engine decomposition:**

1. Create a GroupBox with title.
2. If description provided, create a Label with muted text inside.
3. Create a Flex column content container inside.
4. Set agent hint: "Form section: '{title}'"
5. Mount under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateFormSection",
    assembly_id: u64,
    content_id: u64,
}
```

## 3.6 CreateToolbar

Create a horizontal toolbar region with action buttons.

```rust
AgentAction::CreateToolbar {
    parent_id: u64,
    buttons: Vec<String>,     // Button labels
    index: Option<usize>,
}
```

**Engine decomposition:**

1. Create a Flex row with spacing.
2. Create N Button widgets with labels.
3. Create a FlexSpacer at the end (pushes buttons left).
4. Set agent hints on each button.
5. Mount under `parent_id`.

**Response:**

```rust
AgentEvent::AssemblyResult {
    action: "CreateToolbar",
    assembly_id: u64,
    button_ids: Vec<u64>,
}
```

# 4. Blueprint Annotations

## 4.1 Annotate

Attach a design-intent note to any widget. Annotations are metadata — not rendered, but included in blueprint exports and Snapshot output.

```rust
AgentAction::Annotate {
    widget_id: u64,
    note: String,
}
```

Stored in the widget tree as agent metadata. Visible in compact tree output as `[note: "..."]`.

## 4.2 LinkNavigation

Declare navigation intent between widgets. The source widget "should navigate to" the target when activated. Blueprint metadata only — does not wire runtime callbacks.

```rust
AgentAction::LinkNavigation {
    source_id: u64,         // e.g., a Button
    target_id: u64,         // e.g., a tab content region
    trigger: String,        // "click", "double_click", "enter"
}
```

Stored as annotation metadata. Visible in blueprint exports.

## 4.3 ExportBlueprint

Dump the full widget tree with all annotations, navigation links, and assembly metadata as a structured document.

```rust
AgentAction::ExportBlueprint {
    name: String,
    format: String,  // "compact" or "detailed"
}
```

Returns an `AgentEvent::BlueprintExport` with the full document.

# 5. Compact Tree Format Extension

The compact tree format (used by halcyon_view_tree) is extended to show assembly structure and annotations:

```
[2] Flex.col
  [5] TabLayout ── tabs: Dashboard | Settings | Logs
    [6] TabBar ── [Dashboard] Settings Logs
    [8] Flex.col ── "Tab: Dashboard" [hint: Content region for tab 'Dashboard']
      [10] Label "Welcome to the dashboard"
    [12] Flex.col ── "Tab: Settings" [hint: Content region for tab 'Settings']
      (empty)
    [14] Flex.col ── "Tab: Logs" [hint: Content region for tab 'Logs']
      (empty)
  [16] WindowRegion ── "[Window] Preferences"
    [18] Flex.col [hint: Window region: 'Preferences'. Implement as separate OS window.]
      (empty)
```

Assembly actions mark their root widget with an assembly type tag visible in the tree.

# 6. Invariants

**INV-321-1:** Assembly actions are atomic. Either all child widgets are created successfully, or none are (rollback on failure).

**INV-321-2:** Assembly result IDs are stable for the lifetime of the assembly. Removing any child widget via RemoveWidget invalidates the assembly but does not crash.

**INV-321-3:** Annotations are metadata only. They MUST NOT affect layout, painting, event routing, or any runtime behavior.

**INV-321-4:** Wireframe mode assemblies do NOT wire runtime navigation. All content regions are simultaneously visible. No NavigationHost, no ScreenHost projection, no sink wiring.

**INV-321-5:** Assembly actions reuse existing widgets (TabBar, Breadcrumbs, SplitPane, GroupBox, Flex, Button, SectionHeader, Divider, Label). No new widget types are introduced.

**INV-321-6:** Assembly results use the standard AgentEvent channel. No new communication paths.

# 7. Forbidden Behaviors

**FORBID-321-1:** Assembly actions MUST NOT create runtime navigation wiring in wireframe mode. No NavigationHost instantiation, no sink callbacks, no ScreenHost projection. Wireframe = visual structure only.

**FORBID-321-2:** Annotations MUST NOT be rendered as visible UI elements. They exist in metadata only.

**FORBID-321-3:** Assembly actions MUST NOT introduce new widget types. They compose existing widgets only.

# 8. MCP Bridge Tools

New tools exposed via mcp-bridge for assembly actions:

- **halcyon_create_tab_layout** — params: parent_id, tabs (string array)
- **halcyon_create_wizard** — params: parent_id, steps (string array)
- **halcyon_create_split_layout** — params: parent_id, orientation, ratio
- **halcyon_create_window_region** — params: parent_id, title, width, height
- **halcyon_create_form_section** — params: parent_id, title, description
- **halcyon_create_toolbar** — params: parent_id, buttons (string array)
- **halcyon_annotate** — params: widget_id, note
- **halcyon_export_blueprint** — params: name, format

# 9. Implementation Phases

**Phase 1 — Core assemblies:** CreateTabLayout, CreateSplitLayout, CreateFormSection, CreateWindowRegion. These cover 80% of complex layout needs. Add corresponding MCP tools. Add Annotate action.

**Phase 2 — Flow assemblies:** CreateWizard, CreateToolbar. Add LinkNavigation and ExportBlueprint.

**Phase 3 — Live mode (future):** Wire assemblies to real NavigationHost + ScreenHost for working runtime navigation. Requires callback/event wiring protocol extension.

# 10. Relationship to Existing SPECs

- **SPEC-HAL-228 (NavigationHost):** Wireframe mode does NOT use NavigationHost. Live mode (Phase 3) will wire assembly tab/wizard navigation through NavigationHost.
- **SPEC-HAL-319 (Runtime UI Composition):** Assembly actions are a superset. Widget-level actions remain unchanged. Assembly actions decompose into widget-level operations internally.
- **SPEC-HAL-082 (Multi-Window):** CreateWindowRegion in wireframe mode simulates windows as GroupBox regions. Live mode (future) could create real OS windows.

# 11. Declarative Composition Extensions

`halcyon_compose` (the bulk-compose MCP tool) accepts a JSON tree of widget specs. Two extensions let a single compose call describe layouts that previously needed multiple round-trips.

## 11.1 Named References — `ref` / `for`

The orchestrator assigns widget IDs at materialization time, so a spec written ahead of time can't say "this Label belongs to that Switch." Two optional fields solve this:

- **`"ref": "<name>"`** — assigns a user-defined name to this widget. Stored in the compose call's local ref table.
- **`"config.for": "<name>"`** — references another widget by name. After the entire tree is materialized, the bridge resolves every `for` to the matching ref's widget ID and emits a `ConfigureWidget` with `for_widget_id = target` so the widget can wire itself up.

Example — clicking the Label toggles the Switch:

```json
{
  "widget_type": "Flex",
  "config": {"direction": "row", "spacing": 8},
  "children": [
    {"widget_type": "Label", "config": {"text": "Beta dashboard", "for": "beta"}},
    {"widget_type": "FlexSpacer"},
    {"widget_type": "Switch", "config": {"label": ""}, "ref": "beta"}
  ]
}
```

Resolution semantics:

- A `for` that names no known `ref` is silently ignored — no error.
- A `ref` with no consumer is silently kept — useful for forward references and for tab content slots (see 11.2).
- Refs are scoped to a single `halcyon_compose` call. They do not persist across compose calls.

Widget support today:

- **Label** consumes `for_widget_id` and dispatches a synthetic primary `PointerDown` to the target on click. Switch and Checkbox toggle on `Down`, so this is sufficient.
- Other widgets may opt in by reading `for_widget_id` in their factory branch and acting on it. The bridge always sends the `ConfigureWidget`; ignoring it is allowed.

## 11.2 TabLayout `tab_children`

`CreateTabLayout` returns content area IDs in `AssemblyResult.ids.content_ids` — but those IDs are only known after the call. To populate the tabs in the same compose call, use `tab_children`:

```json
{
  "widget_type": "TabLayout",
  "config": {"tabs": ["General", "Security", "Advanced"]},
  "tab_children": {
    "General":  [ {"widget_type": "Checkbox", "config": {"label": "Dark mode"}} ],
    "Security": [ {"widget_type": "ComboBox", "config": {"options": ["AES-256", "ChaCha20"]}} ],
    "Advanced": [ {"widget_type": "Label",    "config": {"text": "No settings yet", "color": "muted"}} ]
  }
}
```

Compose engine flow when `widget_type == "TabLayout"`:

1. Send `CreateTabLayout { parent_id, tabs }` and read `AssemblyResult`.
2. Extract `assembly_id` and `ids.content_ids` from the response.
3. Append the assembly to `created` with the `content_ids` array surfaced for caller visibility.
4. For each label in `config.tabs`, if `tab_children["<label>"]` exists, recurse those children into `content_ids[i]` (the i-th content area).

Auto-published refs when the TabLayout has a `ref`:

- **`"<ref>"`** → assembly_id (the TabLayout root)
- **`"<ref>:<tab_label>"`** → that tab's content_id

Example (using both extensions together — siblings can target a specific tab's content slot once `parent_ref` lands):

```json
{
  "widget_type": "TabLayout", "ref": "settings",
  "config": {"tabs": ["General", "Security"]},
  "tab_children": {
    "General":  [ {"widget_type": "Checkbox", "config": {"label": "Dark mode"}} ],
    "Security": [ {"widget_type": "Checkbox", "config": {"label": "Two-factor auth"}} ]
  }
}
```

After this call: `settings` resolves to the assembly id, `settings:General` to the General content id, `settings:Security` to the Security content id.

Boundary behavior:

- Tab labels in `tab_children` that don't appear in `config.tabs` are silently ignored.
- Tabs without a `tab_children` entry stay empty (no content widgets added).
- The standard `children` field is NOT used on a TabLayout spec — content slots are filled exclusively via `tab_children`.
- **SPEC-HAL-310 (Navigator):** Navigator's multi-view pattern (Grid/List/Thumbnail modes) is a natural candidate for a future CreateNavigator assembly action.
