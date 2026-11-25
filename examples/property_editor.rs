// =============================================================================
// SPEC-HAL-260A: PropertyEditor v2 — Multi-Column Editing Proof Demo
// =============================================================================
//
// Exercises every `PropertyValue` archetype across FOUR columns. Three are
// editable Data columns — this is the proof that PE supports multi-column
// data value editing across heterogeneous PropertyValue types dispatched
// through the unified per-cell loop (SPEC-HAL-260A §14.3 pass 4) with no
// column-specific structural carve-out (INV-CELL-PAINT-UNIFORM §9.1,
// FORBID-PE-COLUMN-ID-CARVE-OUT §9.2).
//
//   Property (NonData, tree column)
//   Value    (Data — primary value per row; type varies)
//   Default  (Data — initial value per row; same PropertyValue type as Value)
//   Notes    (Data — free-form String annotation per row)
//
// Per-row column types:
//   Application Name → Value=String,  Default=String,  Notes=String
//   Auto-save        → Value=Bool,    Default=Bool,    Notes=String
//   Save Interval    → Value=Integer, Default=Integer, Notes=String
//   Theme            → Value=Choice,  Default=Choice,  Notes=String
//   Accent Color     → Value=Color,   Default=Color,   Notes=String
//   Opacity          → Value=Float,   Default=Float,   Notes=String
//   Bindings/{Save,Quit}
//                    → Value=KeyBinding, Default=KeyBinding, Notes=String
//   Margins/{Left,Right}
//                    → Value=Integer, Default=Integer, Notes=String
//   About/{Version,Build}
//                    → All three cells wrapped in PropertyValue::ReadOnly
//
// Cell-level type independence (Mixed Types section). Rows where Value
// and Default columns deliberately carry DIFFERENT PropertyValue variants:
//   Mixed Types/Integer-Bool   → Value=Integer,Default=Bool,     Notes=String
//   Mixed Types/Color-Float    → Value=Color,  Default=Float,    Notes=String
//   Mixed Types/Choice-String  → Value=Choice, Default=String,   Notes=String
//   Mixed Types/Bool-KeyBind   → Value=Bool,   Default=KeyBinding,Notes=String
// These rows have no real-world semantic meaning — they exist to prove
// PE dispatches each cell's editor from its own
// (row_id, column_id) → PropertyValue lookup, with no row-level or
// column-level type constraint. Each row mounts a different pair of
// editor widgets on the Value and Default cells side-by-side.
//
// ReadOnly signaling (replaces the prior "Type" label column).
//   Per-cell dimming via `is_readonly = !cell.editable
//   || matches!(cell.value, PropertyValue::ReadOnly(_))` paints the cell
//   text in `theme.colors.typography.secondary` (body.rs §14.3 pass 4b).
//   Each ReadOnly cell shows dimmed text without any auxiliary label
//   column. The About section demonstrates row-level ReadOnly: every
//   cell across Value / Default / Notes uses `PropertyValue::ReadOnly(...)`,
//   so the entire row reads as dimmed.
//
// Archetype → editor binding (per SPEC-HAL-260A §6.2):
//   - String       → TextField (SPEC-HAL-013 / 014)
//   - Integer      → SpinBox (SPEC-HAL-212)
//   - Float        → SpinBox (SPEC-HAL-212)
//   - Bool         → always-mounted Checkbox (§6.2; keyed by
//                    (row_id, column_id) so a Bool cell on Value AND a
//                    Bool cell on Default each mount their own Checkbox)
//   - Choice       → ComboBox (SPEC-HAL-201–206)
//   - Color        → ColorPickerField (SPEC-HAL-272)
//   - KeyBinding   → KeybindEditor (SPEC-HAL-329)
//   - ReadOnly     → display-only (dimmed text)
//   - Section      → heading row (§6.1, §14.3)
//
// Tree hierarchy: Section → SubSection (`Margins`) → Children (`Left`, `Right`).
//
// Data flow (SPEC-HAL-260A §3 Data-Layer Sovereignty):
//   - `DemoSource` owns the entire data domain (nodes + cells + revision).
//   - PE reads via `PropertyDataSource::nodes()` and `CellSource::cell()`.
//   - Edits commit through `CellSource::commit_cell`; the source bumps
//     `revision` so subsequent paints observe the new value (live `cell()`
//     reads — no extra revision-driven rebuild needed for cell-value updates).
//
// Demo Law: `handle_input` returns `false`
// for every event. The demo does NOT consume Escape. The framework owns
// Escape for capture release and focus reset.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::layout::FlexParams;
use halcyon::core::paint::Color;
use halcyon::core::widget::Widget;
use halcyon::model::tabular::column::ColumnId;
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

const WINDOW_WIDTH: f32 = 720.0;
const WINDOW_HEIGHT: f32 = 720.0;

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

// =============================================================================
// Demo screen.
// =============================================================================

struct PEv2DemoScreen {
    /// Returned from `Screen::build` after the tree has been mounted once;
    /// the framework's signature requires an `&mut dyn Widget` even though
    /// composition happens via `cx.mount_child` + `cx.tree.add_child`.
    placeholder: Label,
    /// Guards against re-mounting the tree on every frame. PE holds internal
    /// state through `Rc<RefCell<PropertyEditorCore>>`; rebuilding every
    /// frame would discard that state.
    built: bool,
}

impl PEv2DemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for PEv2DemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        // Construct PE empty and configure columns via the SPEC §7.2 builders.
        // Three editable Data columns (Value / Default / Notes) prove
        // multi-column data value editing across heterogeneous
        // PropertyValue types. Per SPEC-HAL-180 INV-COL-ELASTIC-1 the
        // first value column (Value) becomes elastic (flex 1.0);
        // subsequent value columns (Default, Notes) take fixed 200px
        // width from `add_value_column`.
        let mut editor = PropertyEditor::new();
        editor.add_name_column("Property");
        let value_col_id = editor.add_value_column("Value");
        let default_col_id = editor.add_value_column("Default");
        let notes_col_id = editor.add_value_column("Notes");

        // Build the synthetic data source with the allocated column IDs,
        // then register it with PE. PE's `set_data_source` performs the
        // initial-load `rebuild_hts_data` pipeline.
        let source = Rc::new(RefCell::new(DemoSource::new(
            value_col_id,
            default_col_id,
            notes_col_id,
        )));
        editor.set_data_source(source);

        // Extract the body widget before moving the editor into the tree.
        let body_widget = editor.create_body_widget();

        // Tree composition (mirrors v1 PE demo, adapted to v2):
        //   Root (Flex column)
        //     └─ PropertyEditor (flex 1.0)
        //          └─ ScrollArea
        //               └─ PropertyEditorBody
        let root = cx.mount_child(Box::new(Flex::column()));

        let editor_id = cx.tree.add_child(root, Box::new(editor), None, 0);
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

        // Auto-focus PE so keyboard navigation works without an initial click.
        cx.tree.deferred_focus = Some(editor_id);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // DEMO LAW: demos do not consume any
        // input. The framework owns Escape (capture release / focus reset),
        // Tab (focus traversal), and all other framework keys.
        false
    }

    fn mark_rebuild(&mut self) {
        // SPEC-HAL-018 incremental-rebuild pattern: no-op. The `built: bool`
        // guard above ensures the widget tree is constructed exactly once
        // on the first frame; this no-op prevents the engine from setting
        // an internal "needs rebuild" flag that would later cause the
        // subtree to be cleared.
    }

    fn take_rebuild_flag(&mut self) -> bool {
        // SPEC-HAL-018 incremental-rebuild pattern: always return false so
        // the engine never clears and re-asks for the screen subtree.
        // Pairs with the `built: bool` guard in `build` and is required
        // because `PropertyEditorCore` carries interactive state
        // (selection, focused_column, edit_session) that would be
        // destroyed on per-frame reconstruction of `PropertyEditor`.
        false
    }
}

// =============================================================================
// Entry point.
// =============================================================================

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== PropertyEditor v2 Demo (SPEC-HAL-260A) ===");
    println!("Columns: Property (tree) | Value | Default | Notes");
    println!();
    println!("Archetypes demonstrated:");
    println!("  String, Integer, Float, Bool, Choice, Color,");
    println!("  KeyBinding, ReadOnly, Section, Hierarchy (Margins → Left+Right).");
    println!();
    println!("Keyboard (SPEC-HAL-260A §10.5):");
    println!("  h j k l    — row / column navigation");
    println!("  i / Enter  — begin edit on focused cell");
    println!("  Enter      — commit (inside editor)");
    println!("  Escape     — cancel edit / framework-owned outside edit");
    println!("  Space      — toggle Bool / expand-collapse parent / activate");
    println!("  Tab        — commit and advance to next cell");
    println!();
    println!("Bool cells: always-mounted Checkbox (toggle commits inline).");
    println!("Commits log to stderr.");
    println!("Close the window to exit (Escape never exits the demo).");
    println!("===============================================");
    println!();

    let app =
        App::new(AppConfig::new("PropertyEditor v2 Demo").with_size(WINDOW_WIDTH, WINDOW_HEIGHT));
    app.run(PEv2DemoScreen::new());
}
