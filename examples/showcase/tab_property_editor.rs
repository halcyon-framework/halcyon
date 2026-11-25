// Showcase — Property Editor tab content (SPEC-HAL-260A).
//
// Transplanted from examples/property_editor_v2.rs. Identical composition,
// identical data fixture — adapted to the showcase tab pattern: the demo's
// own `Screen` impl is replaced by a `build_property_editor` method that
// adds PE + ScrollArea + body as children of the showcase's content_node.
//
// The standalone `examples/property_editor_v2.rs` is unchanged and remains
// independently runnable. This file is the showcase-embedded view of the
// same content.
//
// Structure (mirrors examples/property_editor_v2.rs:663-685):
//   content_node (passed in by showcase Screen::build)
//     └─ PropertyEditor (flex 1.0)
//          └─ ScrollArea
//               └─ PropertyEditorBody
//
// All four PE archetypes (Property tree column + Value/Default/Notes data
// columns) are exercised. See examples/property_editor_v2.rs:1-76 for the
// archetype contract documentation; not repeated here.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::ShowcaseScreen;
use halcyon::core::app::ScreenFrameContext;
use halcyon::core::event::KeyCode;
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::widget_tree::NodeId;
use halcyon::model::tabular::column::ColumnId;
use halcyon::prelude::*;
use halcyon::runtime::actions::{ActionId, Shortcut};
use halcyon::runtime::binding_registry::{
    Binding, BindingLayer, BindingPriority, BindingRegistry, InputPattern,
};
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::property_editor::{
    Cell, CellSource, PropertyDataSource, PropertyEditor, PropertyNode, PropertyNodeKind,
    PropertyValue, RowArchetype,
};
use halcyon::widgets::scroll_area::ScrollArea;

// =============================================================================
// SPEC-HAL-020C: KeyBinding rows backed by BindingRegistry.
// =============================================================================
//
// Each KeyBinding row in this demo represents an action. Bindings register
// into BindingRegistry at the User layer with Normal priority, tagged with
// the row's static tag so updates can remove + re-add the single binding
// without disturbing other layers.
//
// ActionId range 1000+ is reserved for app-level actions per
// `runtime/actions.rs:31`.

struct ActionRow {
    label: &'static str,
    action_id: ActionId,
    tag: &'static str,
}

const ACTION_ROWS: &[ActionRow] = &[
    ActionRow {
        label: "Save",
        action_id: ActionId(1000),
        tag: "demo_pe_action_save",
    },
    ActionRow {
        label: "Quit",
        action_id: ActionId(1001),
        tag: "demo_pe_action_quit",
    },
    ActionRow {
        label: "Open",
        action_id: ActionId(1002),
        tag: "demo_pe_action_open",
    },
    ActionRow {
        label: "Find",
        action_id: ActionId(1003),
        tag: "demo_pe_action_find",
    },
    ActionRow {
        label: "Replace",
        action_id: ActionId(1004),
        tag: "demo_pe_action_replace",
    },
    ActionRow {
        label: "Refresh",
        action_id: ActionId(1005),
        tag: "demo_pe_action_refresh",
    },
];

fn action_row_by_label(label: &str) -> Option<&'static ActionRow> {
    ACTION_ROWS.iter().find(|a| a.label == label)
}

// =============================================================================
// Synthetic in-memory PropertyDataSource.
// =============================================================================

struct DemoSource {
    /// Tree topology in pre-order traversal (SPEC-HAL-259 INV-HTS-PREORDER).
    nodes: Vec<PropertyNode>,
    /// 2D cell store keyed by `(row_id, column_id)`.
    cells: HashMap<(u64, ColumnId), Cell>,
    /// Monotonic revision; bumped on every successful `commit_cell`.
    revision: u64,
    /// Next available node identifier.
    next_id: u64,
    /// ColumnId for the primary Value column (Data, editable; PropertyValue
    /// type varies per row).
    value_col_id: ColumnId,
    /// ColumnId for the Default column (Data, editable; same PropertyValue
    /// type as Value on a given row).
    default_col_id: ColumnId,
    /// ColumnId for the Notes column (Data, editable; always PropertyValue::String
    /// on editable rows, wrapped in ReadOnly on rows where the primary
    /// Value is itself ReadOnly).
    notes_col_id: ColumnId,
    /// SPEC-HAL-020C: source of truth for binding state. KeyBinding cells
    /// are a VIEW; this is the MODEL. Conflict detection consults this
    /// registry; updates write through it (remove_by_tag + add per row).
    registry: Rc<RefCell<BindingRegistry>>,
    /// row_id → (action_id, tag) for KeyBinding rows. Populated alongside
    /// the cell when the row is added via `add_keybind_property`.
    row_action: HashMap<u64, (ActionId, &'static str)>,
}

impl DemoSource {
    fn new(value_col_id: ColumnId, default_col_id: ColumnId, notes_col_id: ColumnId) -> Self {
        let mut s = Self {
            nodes: Vec::new(),
            cells: HashMap::new(),
            revision: 1,
            next_id: 1,
            value_col_id,
            default_col_id,
            notes_col_id,
            registry: Rc::new(RefCell::new(BindingRegistry::new())),
            row_action: HashMap::new(),
        };
        s.populate();
        s
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_section(&mut self, label: &str) -> u64 {
        let id = self.alloc_id();
        self.nodes.push(PropertyNode {
            node_id: id,
            label: label.into(),
            parent_id: None,
            kind: PropertyNodeKind::Section,
        });
        id
    }

    fn add_property(
        &mut self,
        label: &str,
        parent_id: Option<u64>,
        value: PropertyValue,
        default_value: PropertyValue,
        notes: &str,
    ) -> u64 {
        let id = self.alloc_id();
        let value_editable = value.is_editable();
        let default_editable = default_value.is_editable();
        // Notes inherits row-level read-only from the primary Value:
        // when Value is wrapped in PropertyValue::ReadOnly, the row is
        // logically immutable and the Notes cell mirrors that. Otherwise
        // Notes is a free-form editable String. This produces the desired
        // dimmed-across-the-row visual on About/Version and About/Build
        // without any auxiliary type label column.
        let notes_value = if value_editable {
            PropertyValue::String(notes.into())
        } else {
            PropertyValue::ReadOnly(Box::new(PropertyValue::String(notes.into())))
        };
        let notes_editable = notes_value.is_editable();
        self.nodes.push(PropertyNode {
            node_id: id,
            label: label.into(),
            parent_id,
            kind: PropertyNodeKind::Data,
        });
        self.cells
            .insert((id, self.value_col_id), Cell::new(value, value_editable));
        self.cells.insert(
            (id, self.default_col_id),
            Cell::new(default_value, default_editable),
        );
        self.cells.insert(
            (id, self.notes_col_id),
            Cell::new(notes_value, notes_editable),
        );
        id
    }

    /// Add a SubSection row: a nested grouping band under a Section.
    ///
    /// Per SPEC-HAL-260A §5.5.1, SubSection is one of the three row
    /// archetypes (Data / Section / SubSection). SubSection rows host no
    /// cells — `CellSource::cell()` returns `None` for every column — and
    /// project to `RowArchetype::Band` at the trait boundary via this
    /// source's `row_archetype` override. Distinct from `Section` by
    /// paint: SubSection uses `list_colors.subheader_background` (more
    /// subdued than Section's `header_background`).
    ///
    /// Models the Unity Inspector / VS Properties nested-section pattern.
    /// Structural invariant (INV-ARCHETYPE-HIERARCHY, asserted in
    /// `core::build_node_metas`): `parent_id` must resolve to a Section
    /// node.
    fn add_subsection(&mut self, label: &str, parent_id: Option<u64>) -> u64 {
        let id = self.alloc_id();
        self.nodes.push(PropertyNode {
            node_id: id,
            label: label.into(),
            parent_id,
            kind: PropertyNodeKind::SubSection,
        });
        // No cells inserted — `cell()` returns `None` for every column.
        // PE projects this row to `RowArchetype::Band` per the
        // `CellSource::row_archetype` impl below.
        id
    }

    /// SPEC-HAL-020C: add a KeyBinding row AND register its initial binding
    /// in the BindingRegistry. The row's static tag identifies its single
    /// entry in the registry so future updates can remove + re-add cleanly.
    fn add_keybind_property(
        &mut self,
        action: &'static ActionRow,
        parent_id: Option<u64>,
        shortcut: Shortcut,
        default_shortcut: Shortcut,
        notes: &str,
    ) -> u64 {
        // Initial values are always bound — KeyBinding(Some(shortcut)).
        // The unbound variant arises only via the steal flow in
        // commit_cell, where the stolen-from cell is set to None.
        let row_id = self.add_property(
            action.label,
            parent_id,
            PropertyValue::KeyBinding(Some(shortcut)),
            PropertyValue::KeyBinding(Some(default_shortcut)),
            notes,
        );
        self.row_action
            .insert(row_id, (action.action_id, action.tag));
        self.registry.borrow_mut().add(Binding {
            pattern: InputPattern::from(&shortcut),
            action_id: action.action_id,
            layer: BindingLayer::User,
            priority: BindingPriority::Normal,
            tag: Some(action.tag),
        });
        row_id
    }

    fn populate(&mut self) {
        // ---- Section: General -------------------------------------------------
        let general = self.add_section("General");
        self.add_property(
            "Application Name",
            Some(general),
            PropertyValue::String("Halcyon".into()),
            PropertyValue::String("MyApp".into()),
            "Display name shown in window title bar",
        );
        self.add_property(
            "Auto-save",
            Some(general),
            PropertyValue::Bool(true),
            PropertyValue::Bool(false),
            "Enable periodic background backups",
        );
        self.add_property(
            "Save Interval (s)",
            Some(general),
            PropertyValue::Integer(300),
            PropertyValue::Integer(60),
            "Auto-save period in seconds",
        );

        // ---- Section: Appearance ---------------------------------------------
        let appearance = self.add_section("Appearance");
        self.add_property(
            "Theme",
            Some(appearance),
            PropertyValue::Choice {
                selected: 1,
                options: vec!["Light".into(), "Dark".into(), "System".into()],
            },
            PropertyValue::Choice {
                selected: 2,
                options: vec!["Light".into(), "Dark".into(), "System".into()],
            },
            "UI color scheme preference",
        );
        self.add_property(
            "Accent Color",
            Some(appearance),
            PropertyValue::Color(Color::from_hex(0x4A90E2)),
            PropertyValue::Color(Color::from_hex(0x808080)),
            "Primary UI accent color",
        );
        self.add_property(
            "Opacity",
            Some(appearance),
            PropertyValue::Float(0.95),
            PropertyValue::Float(1.0),
            "Window translucency (0.0 transparent — 1.0 opaque)",
        );

        // ---- Section: Bindings -----------------------------------------------
        // SPEC-HAL-020C: each binding row registers into BindingRegistry
        // via add_keybind_property. Conflict detection consults the
        // registry — not the cell map — at commit time and conflict_check
        // time.
        let bindings = self.add_section("Bindings");
        let save_action = action_row_by_label("Save").expect("ACTION_ROWS missing Save");
        let quit_action = action_row_by_label("Quit").expect("ACTION_ROWS missing Quit");
        self.add_keybind_property(
            save_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('s'), true, false, false, false),
            Shortcut::new(KeyCode::Character('s'), true, false, false, false),
            "Save current document",
        );
        self.add_keybind_property(
            quit_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('q'), true, false, false, false),
            Shortcut::new(KeyCode::Character('q'), true, false, false, false),
            "Exit the application",
        );
        let open_action = action_row_by_label("Open").expect("ACTION_ROWS missing Open");
        let find_action = action_row_by_label("Find").expect("ACTION_ROWS missing Find");
        let replace_action = action_row_by_label("Replace").expect("ACTION_ROWS missing Replace");
        let refresh_action = action_row_by_label("Refresh").expect("ACTION_ROWS missing Refresh");
        self.add_keybind_property(
            open_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('o'), true, false, false, false),
            Shortcut::new(KeyCode::Character('o'), true, false, false, false),
            "Open a document",
        );
        self.add_keybind_property(
            find_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('f'), true, false, false, false),
            Shortcut::new(KeyCode::Character('f'), true, false, false, false),
            "Find in current document",
        );
        self.add_keybind_property(
            replace_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('h'), true, true, false, false),
            Shortcut::new(KeyCode::Character('h'), true, true, false, false),
            "Find and replace",
        );
        self.add_keybind_property(
            refresh_action,
            Some(bindings),
            Shortcut::new(KeyCode::Character('r'), true, false, false, false),
            Shortcut::new(KeyCode::Character('r'), true, false, false, false),
            "Refresh the current view",
        );

        // ---- Section: Layout (3-level hierarchy: Section → SubSection → Children)
        // Margins is a SubSection grouping — children Left/Right carry the
        // real values; Margins itself has no own cells (band archetype)
        // per SPEC-HAL-260A §3 Data-Layer Sovereignty.
        let layout = self.add_section("Layout");
        let margins = self.add_subsection("Margins", Some(layout));
        self.add_property(
            "Left",
            Some(margins),
            PropertyValue::Integer(8),
            PropertyValue::Integer(0),
            "Pixel offset from left edge",
        );
        self.add_property(
            "Right",
            Some(margins),
            PropertyValue::Integer(8),
            PropertyValue::Integer(0),
            "Pixel offset from right edge",
        );

        // ---- Section: Mixed Types (per-cell type independence proof) ---------
        // Rows here deliberately carry DIFFERENT PropertyValue variants in
        // Value vs Default columns. They have no real-world semantic
        // meaning — they exist to prove that PE dispatches each cell's
        // editor from its own (row_id, column_id) → PropertyValue lookup,
        // with no row-level or column-level type constraint. Each row
        // mounts a different pair of editor widgets on the Value and
        // Default cells side-by-side.
        let mixed = self.add_section("Mixed Types");
        self.add_property(
            "Integer / Bool",
            Some(mixed),
            PropertyValue::Integer(42),
            PropertyValue::Bool(true),
            "Value=SpinBox, Default=Checkbox",
        );
        self.add_property(
            "Color / Float",
            Some(mixed),
            PropertyValue::Color(Color::from_hex(0xFF6B6B)),
            PropertyValue::Float(0.7),
            "Value=ColorPickerField, Default=SpinBox",
        );
        self.add_property(
            "Choice / String",
            Some(mixed),
            PropertyValue::Choice {
                selected: 0,
                options: vec!["Manual".into(), "Auto".into(), "Disabled".into()],
            },
            PropertyValue::String("custom".into()),
            "Value=ComboBox, Default=TextField",
        );
        self.add_property(
            "Bool / KeyBinding",
            Some(mixed),
            PropertyValue::Bool(false),
            PropertyValue::KeyBinding(Some(Shortcut::new(
                KeyCode::Character('f'),
                true,
                false,
                false,
                false,
            ))),
            "Value=Checkbox, Default=KeybindEditor",
        );

        // ---- Section: Network ------------------------------------------------
        let network = self.add_section("Network");
        self.add_property(
            "Server URL",
            Some(network),
            PropertyValue::String("https://api.example.com".into()),
            PropertyValue::String("https://localhost:8080".into()),
            "Base URL for the remote endpoint",
        );
        self.add_property(
            "Port",
            Some(network),
            PropertyValue::Integer(443),
            PropertyValue::Integer(80),
            "TCP port for outbound connections",
        );
        self.add_property(
            "Timeout (s)",
            Some(network),
            PropertyValue::Integer(30),
            PropertyValue::Integer(10),
            "Request timeout in seconds",
        );
        self.add_property(
            "Use TLS",
            Some(network),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Encrypt outbound traffic",
        );
        self.add_property(
            "Verify Certificate",
            Some(network),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Reject connections with invalid certificates",
        );
        self.add_property(
            "Connection Mode",
            Some(network),
            PropertyValue::Choice {
                selected: 0,
                options: vec!["Direct".into(), "Proxy".into(), "Tunnel".into()],
            },
            PropertyValue::Choice {
                selected: 0,
                options: vec!["Direct".into(), "Proxy".into(), "Tunnel".into()],
            },
            "How outbound traffic reaches the server",
        );

        let proxy = self.add_subsection("Proxy", Some(network));
        self.add_property(
            "Host",
            Some(proxy),
            PropertyValue::String("proxy.internal".into()),
            PropertyValue::String("".into()),
            "Proxy server hostname",
        );
        self.add_property(
            "Port",
            Some(proxy),
            PropertyValue::Integer(8888),
            PropertyValue::Integer(8080),
            "Proxy server port",
        );
        self.add_property(
            "Authentication",
            Some(proxy),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Require proxy credentials",
        );

        // ---- Section: Performance --------------------------------------------
        let performance = self.add_section("Performance");
        self.add_property(
            "Worker Threads",
            Some(performance),
            PropertyValue::Integer(8),
            PropertyValue::Integer(4),
            "Pool size for background work",
        );
        self.add_property(
            "Max Memory (MB)",
            Some(performance),
            PropertyValue::Integer(4096),
            PropertyValue::Integer(2048),
            "Soft cap on process working set",
        );
        self.add_property(
            "Cache Size (MB)",
            Some(performance),
            PropertyValue::Integer(512),
            PropertyValue::Integer(128),
            "Disk-backed cache budget",
        );
        self.add_property(
            "Enable JIT",
            Some(performance),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Use just-in-time compilation paths",
        );
        self.add_property(
            "GC Strategy",
            Some(performance),
            PropertyValue::Choice {
                selected: 1,
                options: vec![
                    "Conservative".into(),
                    "Generational".into(),
                    "Incremental".into(),
                ],
            },
            PropertyValue::Choice {
                selected: 1,
                options: vec![
                    "Conservative".into(),
                    "Generational".into(),
                    "Incremental".into(),
                ],
            },
            "Garbage collector mode",
        );

        let threading = self.add_subsection("Threading", Some(performance));
        self.add_property(
            "I/O Threads",
            Some(threading),
            PropertyValue::Integer(2),
            PropertyValue::Integer(1),
            "Threads dedicated to blocking I/O",
        );
        self.add_property(
            "Compute Threads",
            Some(threading),
            PropertyValue::Integer(6),
            PropertyValue::Integer(2),
            "Threads dedicated to compute work",
        );
        self.add_property(
            "Priority",
            Some(threading),
            PropertyValue::Choice {
                selected: 1,
                options: vec!["Low".into(), "Normal".into(), "High".into()],
            },
            PropertyValue::Choice {
                selected: 1,
                options: vec!["Low".into(), "Normal".into(), "High".into()],
            },
            "OS scheduling priority for worker threads",
        );

        // ---- Section: Editor -------------------------------------------------
        let editor = self.add_section("Editor");
        self.add_property(
            "Font Family",
            Some(editor),
            PropertyValue::String("JetBrains Mono".into()),
            PropertyValue::String("Monospace".into()),
            "Editor monospace font",
        );
        self.add_property(
            "Font Size",
            Some(editor),
            PropertyValue::Integer(14),
            PropertyValue::Integer(13),
            "Editor font size in points",
        );
        self.add_property(
            "Line Height",
            Some(editor),
            PropertyValue::Float(1.4),
            PropertyValue::Float(1.2),
            "Multiplier applied to font size for line height",
        );
        self.add_property(
            "Show Line Numbers",
            Some(editor),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Display gutter line numbers",
        );
        self.add_property(
            "Word Wrap",
            Some(editor),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Wrap long lines at viewport edge",
        );
        self.add_property(
            "Highlight Current Line",
            Some(editor),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Tint the row containing the cursor",
        );
        self.add_property(
            "Cursor Style",
            Some(editor),
            PropertyValue::Choice {
                selected: 2,
                options: vec!["Block".into(), "Underline".into(), "Line".into()],
            },
            PropertyValue::Choice {
                selected: 2,
                options: vec!["Block".into(), "Underline".into(), "Line".into()],
            },
            "Shape of the text cursor",
        );
        self.add_property(
            "Cursor Blink",
            Some(editor),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Animate the cursor opacity",
        );

        let indentation = self.add_subsection("Indentation", Some(editor));
        self.add_property(
            "Tab Width",
            Some(indentation),
            PropertyValue::Integer(4),
            PropertyValue::Integer(4),
            "Visual width of a hard tab",
        );
        self.add_property(
            "Insert Spaces",
            Some(indentation),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Use spaces when Tab is pressed",
        );
        self.add_property(
            "Auto Indent",
            Some(indentation),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Continue indentation on new lines",
        );

        // ---- Section: Privacy ------------------------------------------------
        let privacy = self.add_section("Privacy");
        self.add_property(
            "Telemetry",
            Some(privacy),
            PropertyValue::Bool(false),
            PropertyValue::Bool(true),
            "Report usage metrics to vendor",
        );
        self.add_property(
            "Crash Reports",
            Some(privacy),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Upload crash dumps automatically",
        );
        self.add_property(
            "Usage Statistics",
            Some(privacy),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Collect anonymized feature-usage counts",
        );
        self.add_property(
            "Share Anonymous Data",
            Some(privacy),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Permit aggregated metrics in public reports",
        );

        // ---- Section: Notifications ------------------------------------------
        let notifications = self.add_section("Notifications");
        self.add_property(
            "Desktop Notifications",
            Some(notifications),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Show OS-level toast notifications",
        );
        self.add_property(
            "Sound",
            Some(notifications),
            PropertyValue::Bool(false),
            PropertyValue::Bool(true),
            "Play a sound on notification arrival",
        );
        self.add_property(
            "Level",
            Some(notifications),
            PropertyValue::Choice {
                selected: 0,
                options: vec!["All".into(), "Errors Only".into(), "None".into()],
            },
            PropertyValue::Choice {
                selected: 0,
                options: vec!["All".into(), "Errors Only".into(), "None".into()],
            },
            "Which notification severities to display",
        );
        self.add_property(
            "Frequency Limit (s)",
            Some(notifications),
            PropertyValue::Integer(5),
            PropertyValue::Integer(0),
            "Minimum interval between consecutive notifications",
        );

        // ---- Section: Display ------------------------------------------------
        let display = self.add_section("Display");
        self.add_property(
            "Resolution",
            Some(display),
            PropertyValue::Choice {
                selected: 1,
                options: vec![
                    "1920×1080".into(),
                    "2560×1440".into(),
                    "3840×2160".into(),
                    "Custom".into(),
                ],
            },
            PropertyValue::Choice {
                selected: 0,
                options: vec![
                    "1920×1080".into(),
                    "2560×1440".into(),
                    "3840×2160".into(),
                    "Custom".into(),
                ],
            },
            "Render target resolution",
        );
        self.add_property(
            "Fullscreen",
            Some(display),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Run in exclusive fullscreen mode",
        );
        self.add_property(
            "VSync",
            Some(display),
            PropertyValue::Bool(true),
            PropertyValue::Bool(true),
            "Synchronize buffer swap to refresh rate",
        );
        self.add_property(
            "Refresh Rate (Hz)",
            Some(display),
            PropertyValue::Integer(120),
            PropertyValue::Integer(60),
            "Target frames per second",
        );

        let color_mgmt = self.add_subsection("Color Management", Some(display));
        self.add_property(
            "Profile",
            Some(color_mgmt),
            PropertyValue::Choice {
                selected: 0,
                options: vec!["sRGB".into(), "Display P3".into(), "Adobe RGB".into()],
            },
            PropertyValue::Choice {
                selected: 0,
                options: vec!["sRGB".into(), "Display P3".into(), "Adobe RGB".into()],
            },
            "ICC color profile applied to output",
        );
        self.add_property(
            "Gamma",
            Some(color_mgmt),
            PropertyValue::Float(2.2),
            PropertyValue::Float(2.2),
            "Output gamma curve exponent",
        );
        self.add_property(
            "HDR Enabled",
            Some(color_mgmt),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Enable high dynamic range output",
        );

        // ---- Section: Advanced -----------------------------------------------
        let advanced = self.add_section("Advanced");
        self.add_property(
            "Experimental Features",
            Some(advanced),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Expose in-development features",
        );
        self.add_property(
            "Beta Channel",
            Some(advanced),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Receive pre-release updates",
        );
        self.add_property(
            "Developer Mode",
            Some(advanced),
            PropertyValue::Bool(false),
            PropertyValue::Bool(false),
            "Enable internal diagnostic surfaces",
        );
        self.add_property(
            "Log Level",
            Some(advanced),
            PropertyValue::Choice {
                selected: 2,
                options: vec![
                    "Error".into(),
                    "Warn".into(),
                    "Info".into(),
                    "Debug".into(),
                    "Trace".into(),
                ],
            },
            PropertyValue::Choice {
                selected: 1,
                options: vec![
                    "Error".into(),
                    "Warn".into(),
                    "Info".into(),
                    "Debug".into(),
                    "Trace".into(),
                ],
            },
            "Minimum severity recorded to the log file",
        );

        // ---- Section: About (row-level ReadOnly across all Data columns) -----
        // Every cell on these rows is wrapped in `PropertyValue::ReadOnly`.
        // The dimmed-text treatment (`is_readonly = !cell.editable
        // || matches!(cell.value, PropertyValue::ReadOnly(_))` →
        // `theme.colors.typography.secondary`) signals row-level read-only
        // across the entire row — no auxiliary type label column required.
        // `add_property` automatically wraps the Notes cell in ReadOnly
        // when the primary Value is ReadOnly (see fn body).
        let about = self.add_section("About");
        self.add_property(
            "Version",
            Some(about),
            PropertyValue::ReadOnly(Box::new(PropertyValue::String("1.0.0".into()))),
            PropertyValue::ReadOnly(Box::new(PropertyValue::String("1.0.0".into()))),
            "Auto-set at build time",
        );
        self.add_property(
            "Build",
            Some(about),
            PropertyValue::ReadOnly(Box::new(PropertyValue::Integer(12345))),
            PropertyValue::ReadOnly(Box::new(PropertyValue::Integer(0))),
            "Auto-set at build time",
        );
    }
}

impl CellSource for DemoSource {
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<Cell> {
        self.cells.get(&(row_id, column_id)).cloned()
    }

    /// SPEC-HAL-260A §5.5.1 — project PE's three-variant
    /// `PropertyNodeKind` (Data / Section / SubSection) to the core
    /// binary `RowArchetype` (Data / Band). Section and SubSection both
    /// project to Band; Data projects to Data. Unknown row ids project
    /// to Data per the `CellSource::row_archetype` default.
    fn row_archetype(&self, row_id: u64) -> RowArchetype {
        self.nodes
            .iter()
            .find(|n| n.node_id == row_id)
            .map_or(RowArchetype::Data, |n| match n.kind {
                PropertyNodeKind::Data => RowArchetype::Data,
                PropertyNodeKind::Section | PropertyNodeKind::SubSection => RowArchetype::Band,
            })
    }

    fn commit_cell(&mut self, row_id: u64, column_id: ColumnId, new_value: PropertyValue) -> bool {
        eprintln!(
            "[DemoSource] commit_cell row_id={} column_id={:?} value={}",
            row_id,
            column_id,
            new_value.display_text(),
        );

        // JC directive 2026-05-21 (steal-binding):
        //   KeyBinding(Some(s)):
        //     - If s is already bound to a DIFFERENT action in the
        //       registry, that action's cell is set to KeyBinding(None)
        //       and its registry entry removed (the steal).
        //     - This row's existing entry is removed; the new entry is
        //       added with this row's tag.
        //     - The new cell value (Some(s)) replaces the previous.
        //   KeyBinding(None):
        //     - This row's registry entry is removed (clear / unbind).
        //     - The cell becomes KeyBinding(None).
        //
        // Commit always succeeds. No rejection path. SPEC-HAL-020C
        // remains the source of truth for binding state; the cell map
        // mirrors it.
        if let PropertyValue::KeyBinding(opt) = &new_value {
            let Some(&(this_action_id, this_tag)) = self.row_action.get(&row_id) else {
                // KeyBinding cell outside ACTION_ROWS — programmer error.
                // Reject defensively rather than corrupt the registry.
                eprintln!(
                    "[DemoSource] commit_cell REJECTED — KeyBinding row {} has no action mapping",
                    row_id
                );
                return false;
            };

            match opt {
                Some(shortcut) => {
                    let pattern = InputPattern::from(shortcut);
                    let prior_owner = self.registry.borrow_mut().lookup(&pattern);

                    // Steal: if another action currently owns this
                    // shortcut, clear that cell + that registry entry.
                    if let Some(other_action_id) = prior_owner {
                        if other_action_id != this_action_id {
                            let other_row = self.row_action.iter().find_map(|(rid, (aid, tag))| {
                                (*aid == other_action_id).then_some((*rid, *tag))
                            });
                            if let Some((other_row_id, other_tag)) = other_row {
                                eprintln!(
                                    "[DemoSource] commit_cell STEAL — shortcut {} taken from row {} (tag {})",
                                    shortcut.to_display_string(),
                                    other_row_id,
                                    other_tag,
                                );
                                self.registry.borrow_mut().remove_by_tag(other_tag);
                                self.cells.insert(
                                    (other_row_id, self.value_col_id),
                                    Cell::new(PropertyValue::KeyBinding(None), true),
                                );
                            }
                        }
                    }

                    // Update this row: remove old tag, add new binding.
                    let mut registry = self.registry.borrow_mut();
                    registry.remove_by_tag(this_tag);
                    registry.add(Binding {
                        pattern,
                        action_id: this_action_id,
                        layer: BindingLayer::User,
                        priority: BindingPriority::Normal,
                        tag: Some(this_tag),
                    });
                }
                None => {
                    // Clear: remove this row's registry entry. Cell will
                    // be updated below to KeyBinding(None).
                    self.registry.borrow_mut().remove_by_tag(this_tag);
                }
            }
        }

        let editable = new_value.is_editable();
        self.cells
            .insert((row_id, column_id), Cell::new(new_value, editable));
        self.revision += 1;
        true
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

impl PropertyDataSource for DemoSource {
    fn nodes(&self) -> Vec<PropertyNode> {
        self.nodes
            .iter()
            .map(|n| PropertyNode {
                node_id: n.node_id,
                label: n.label.clone(),
                parent_id: n.parent_id,
                kind: n.kind,
            })
            .collect()
    }
}

impl ShowcaseScreen {
    /// Build the Property Editor tab content.
    ///
    /// Mirrors examples/property_editor_v2.rs:643-685 verbatim,
    /// substituting `parent` (the showcase's content_node) for the demo's
    /// `cx.mount_child(Box::new(Flex::column()))` root. Same column
    /// configuration (Property + Value + Default + Notes), same DemoSource,
    /// same PE → ScrollArea → body wiring.
    ///
    /// Called from `ShowcaseScreen::build` when `current_section ==
    /// Section::PropertyEditor`. Rebuilt on every tab switch into Property
    /// Editor — DemoSource is reconstructed from scratch each time. This
    /// matches the cold-start behavior of the standalone demo.
    pub(super) fn build_property_editor(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        // Construct PE empty and configure columns via SPEC-HAL-260A §7.2
        // builders. The Notes column carries free-text descriptions
        // significantly wider than the 200px default; override the
        // first-value-column-elastic convention so Notes absorbs the
        // leftover width and Value / Default stay at the fixed 200px
        // default. SPEC-HAL-180 INV-COL-ELASTIC-1 is preserved by
        // `set_column_elastic`, which clears flex on every other
        // registered column before setting the target.
        let mut editor = PropertyEditor::new();
        editor.add_name_column("Property");
        let value_col_id = editor.add_value_column("Value");
        let default_col_id = editor.add_value_column("Default");
        let notes_col_id = editor.add_value_column("Notes");
        editor.set_column_elastic(notes_col_id);

        let source = Rc::new(RefCell::new(DemoSource::new(
            value_col_id,
            default_col_id,
            notes_col_id,
        )));
        editor.set_data_source(source);

        let body_widget = editor.create_body_widget();

        let editor_id = cx.tree.add_child(parent, Box::new(editor), None, 0);
        cx.tree.set_flex(editor_id, FlexParams::flexible(1.0));

        let scroll_area = ScrollArea::new_unwired().enable_horizontal_scrollbar(true);
        let scroll_area_id = cx.tree.add_child(editor_id, Box::new(scroll_area), None, 0);

        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }
    }
}
