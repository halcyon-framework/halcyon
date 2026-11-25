// =============================================================================
// SPEC-HAL-260A §6.3: Default cell-editor factory
// =============================================================================
//
// Shared widget-tier home for the default `CellEditorFactory`. The trait is
// defined at the engine tier (`src/engines/cell_source/mod.rs`) so the
// cell-mechanics substrate can hold a `Box<dyn CellEditorFactory>` without
// depending on widget-tier types; the construction builds widget-tier editors
// (TextField, ComboBox, ColorPickerField, KeybindEditor), so the default
// implementation lives here. Same engine-trait / widget-impl split as
// `CellSource`. Lifted verbatim out of PE v2's `editor_registry` so a second
// cell-bearing consumer can share the same `PropertyValue` → editor mapping.
//
// Per-editor mapping (SPEC §6.3 table, lines 361–372):
//
// | Editor             | Trigger                         | EditExitReason |
// |--------------------|---------------------------------|----------------|
// | TextField          | Enter                           | Commit         |
// | TextField          | Escape                          | Cancel         |
// | TextField          | Tab / Shift+Tab                 | (consumer) *   |
// | ComboBox           | option commit (resolver)        | Commit         |
// | ComboBox           | Escape / blur (no commit)       | Cancel         |
// | ColorPickerField   | OK button                       | Commit         |
// | ColorPickerField   | Cancel / outside / Escape       | Cancel         |
// | KeybindEditor      | one-shot key capture            | Commit         |
// | KeybindEditor      | Backspace-clear (no mods)       | Cancel **      |
// | KeybindEditor      | Escape / Tab / blur (no commit) | Cancel         |
//
//   *  The consumer body intercepts Tab at parent level before forwarding
//      events to the editor child. TextField has no on_tab hook.
//   ** PropertyValue::KeyBinding(Shortcut) has no empty variant, so the
//      widget's clear-intent (on_change(None)) routes through Cancel.
// ***  Framework-initiated capture release without a key-commit
//      (Escape / Tab / blur during capture) fires KeybindEditor's on_cancel
//      callback. The consumer wires on_cancel to EditExitReason::Cancel.
//      on_change and on_cancel are mutually exclusive per session per the
//      widget's committed_in_session guard.

use std::rc::Rc;

use crate::core::paint::Color;
use crate::core::widget::Widget;
use crate::model::default_selection_resolver::DefaultSelectionResolver;
use crate::model::selection_resolver::{CommittedValue, OptionId, ResolverHandle};
use crate::model::tabular::column::ColumnId;
use crate::runtime::actions::Shortcut;
use crate::widgets::checkbox::Checkbox;
use crate::widgets::color_picker_field::ColorPickerField;
use crate::widgets::combo_box::ComboBox;
use crate::widgets::keybind_editor::KeybindEditor;
use crate::widgets::text_field::TextField;

use crate::engines::cell_source::{
    CellEditorFactory, ComboBoxCellEditorSpec, EditExitReason, PropertyValue,
};

/// The standard `CellEditorFactory`: maps each editable `PropertyValue`
/// variant to its editor widget. Stateless unit struct.
pub(crate) struct DefaultCellEditorFactory;

/// HAL391's closed family adapter. Public factory signatures and custom
/// factories remain unchanged; an unknown Widget does not acquire a surface.
pub(crate) fn cell_editor_visual_facts(
    widget: &dyn Widget,
) -> Option<crate::core::appearance::CellEditorVisualFacts> {
    let widget = widget.as_any();
    if let Some(field) = widget.downcast_ref::<TextField>() {
        Some(field.cell_editor_visual_facts())
    } else if let Some(combo) = widget.downcast_ref::<ComboBox>() {
        Some(combo.cell_editor_visual_facts())
    } else if let Some(spin) = widget.downcast_ref::<crate::widgets::spin_box::SpinBox>() {
        Some(spin.cell_editor_visual_facts())
    } else if let Some(field) = widget.downcast_ref::<ColorPickerField>() {
        Some(field.cell_editor_visual_facts())
    } else {
        widget.downcast_ref::<Checkbox>().map(Checkbox::cell_editor_visual_facts)
    }
}

impl DefaultCellEditorFactory {
    pub(crate) fn new() -> Self {
        Self
    }
}

impl CellEditorFactory for DefaultCellEditorFactory {
    fn create_bool_editor(
        &self,
        checked: bool,
        disabled: bool,
        _cell_key: (u64, ColumnId),
        on_toggle: Box<dyn Fn(bool)>,
    ) -> Box<dyn Widget> {
        let mut cb = Checkbox::checked(checked).on_toggle_changed(move |new_val| {
            on_toggle(new_val);
        });
        cb.set_disabled(disabled);
        Box::new(cb)
    }

    fn sync_bool_editor(&self, widget: &mut dyn Widget, checked: bool, disabled: bool) {
        if let Some(cb) = widget.as_any_mut().downcast_mut::<Checkbox>() {
            if cb.is_checked() != checked {
                cb.set_checked(checked);
            }
            if cb.is_disabled() != disabled {
                cb.set_disabled(disabled);
            }
        }
    }

    fn create_combo_box_editor(
        &self,
        spec: &ComboBoxCellEditorSpec,
        _cell_key: (u64, ColumnId),
        on_commit: Box<dyn Fn(String)>,
    ) -> Box<dyn Widget> {
        let mut cb = ComboBox::simple_with_values(spec.options.clone());
        cb.set_text_input_enabled(false);
        cb.set_input_text(spec.current_value.clone())
            .expect("new ComboBox accepts construction props before mounting");
        cb.set_factory_value_callback(on_commit);
        Box::new(cb)
    }

    fn sync_combo_box_editor(
        &self,
        owner: &mut crate::core::composite_text_edit::CompositeEditorOwnerContext<'_>,
        target: &crate::core::composite_text_edit::CompositeEditorId,
        spec: ComboBoxCellEditorSpec,
    ) -> crate::core::composite_text_edit::CompositeEditReply {
        owner.sync_combo_box(target, spec)
    }

    /// SPEC-HAL-260A §6.3: mount-on-demand. Given a `PropertyValue`, returns
    /// the corresponding editor widget with `on_exit` wired into the editor's
    /// commit/cancel callbacks. Panics for variants that are not
    /// editor-mounted: `Bool` is always-mounted (§6.2); `ReadOnly` and `Badge`
    /// are non-editable. The consumer's edit-cycle gate rejects non-editable
    /// cells before calling here.
    fn create_editor(
        &self,
        value: &PropertyValue,
        cell_key: (u64, ColumnId),
        on_exit: Box<dyn Fn(EditExitReason, Option<PropertyValue>)>,
    ) -> Box<dyn Widget> {
        // Convert the Box<dyn Fn> into an Rc so individual make_* constructors
        // can clone the handle for each callback closure they wire.
        let on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)> = Rc::from(on_exit);
        match value {
            PropertyValue::String(s) => Box::new(make_text_field(s, cell_key, on_exit)),
            PropertyValue::Integer(n) => Box::new(make_integer_field(*n, cell_key, on_exit)),
            PropertyValue::Float(f) => Box::new(make_float_field(*f, cell_key, on_exit)),
            PropertyValue::Choice { selected, options } => Box::new(make_combo_box(
                *selected,
                options.clone(),
                cell_key,
                on_exit,
            )),
            PropertyValue::Color(c) => Box::new(make_color_picker_field(*c, cell_key, on_exit)),
            // KeyBinding carries Option<Shortcut> — None = unbound. The editor
            // mounts with that value and the user can either capture a new
            // shortcut (commits Some) or press Backspace to clear (commits
            // None — see make_keybind_editor's on_change routing).
            PropertyValue::KeyBinding(s) => Box::new(make_keybind_editor(*s, cell_key, on_exit)),
            PropertyValue::Bool(_) => panic!("Bool is always-mounted; not editor-produced"),
            PropertyValue::ReadOnly(_) => panic!("ReadOnly is non-editable"),
            PropertyValue::Badge(_, _) => panic!("Badge is non-editable"),
        }
    }
}

// ----------------------------------------------------------------------------
// Per-variant constructors
// ----------------------------------------------------------------------------

/// TextField for `PropertyValue::String`. Wires Enter→Commit and
/// Escape→Cancel. Tab/Shift+Tab → CommitAndAdvance is handled at the PE
/// Body parent level (Body intercepts Tab before forwarding to the editor
/// child). `select_all_on_capture` is enabled for spreadsheet-style cell
/// editing — entering capture selects all so typing replaces.
fn make_text_field(
    initial: &str,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> TextField {
    let mut tf = TextField::new().with_text(initial);
    tf.set_select_all_on_capture(true);
    let on_exit_submit = on_exit.clone();
    tf.set_on_submit(move |text: &str| {
        on_exit_submit(
            EditExitReason::Commit,
            Some(PropertyValue::String(text.to_string())),
        );
    });
    let on_exit_cancel = on_exit;
    tf.set_on_cancel(move || {
        on_exit_cancel(EditExitReason::Cancel, None);
    });
    tf
}

/// Integer editor for `PropertyValue::Integer(i64)`. TextField with an
/// integer input filter per SPEC-HAL-260A §6.3. Wires Enter → Commit
/// (parsing the typed text as `i64`); on parse failure of typed Enter,
/// routes Cancel. Escape → Cancel.
fn make_integer_field(
    initial: i64,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> TextField {
    let mut tf = TextField::new().with_text(format!("{}", initial));
    tf.set_select_all_on_capture(true);
    tf.set_input_filter(|c: char| c.is_ascii_digit() || c == '-' || c == '+');
    let on_exit_submit = on_exit.clone();
    tf.set_on_submit(move |text: &str| match text.trim().parse::<i64>() {
        Ok(v) => on_exit_submit(EditExitReason::Commit, Some(PropertyValue::Integer(v))),
        Err(_) => on_exit_submit(EditExitReason::Cancel, None),
    });
    let on_exit_cancel = on_exit;
    tf.set_on_cancel(move || {
        on_exit_cancel(EditExitReason::Cancel, None);
    });
    tf
}

/// Float editor for `PropertyValue::Float(f64)`. TextField with a
/// numeric input filter per SPEC-HAL-260A §6.3. Wires Enter → Commit
/// (parsing the typed text as `f64`); on parse failure of typed Enter,
/// routes Cancel. Escape → Cancel.
fn make_float_field(
    initial: f64,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> TextField {
    // Seed from the same authority the cell display uses (PropertyValue::
    // display_text), so the editor text matches the painted value instead of
    // re-deriving its own float format (SPEC-PM-009).
    let mut tf = TextField::new().with_text(PropertyValue::Float(initial).display_text());
    tf.set_select_all_on_capture(true);
    tf.set_input_filter(|c: char| {
        c.is_ascii_digit() || c == '.' || c == '-' || c == '+' || c == 'e' || c == 'E'
    });
    let on_exit_submit = on_exit.clone();
    tf.set_on_submit(move |text: &str| match text.trim().parse::<f64>() {
        Ok(v) => on_exit_submit(EditExitReason::Commit, Some(PropertyValue::Float(v))),
        Err(_) => on_exit_submit(EditExitReason::Cancel, None),
    });
    let on_exit_cancel = on_exit;
    tf.set_on_cancel(move || {
        on_exit_cancel(EditExitReason::Cancel, None);
    });
    tf
}

/// ComboBox for `PropertyValue::Choice`. Options are registered with
/// `OptionId::from_raw(i)` so the OptionId's raw value IS the index into
/// the options vec — `make_combo_box` reads it back via
/// `OptionId::raw() as usize`. Wires the ComboBox callbacks:
/// `on_committed` → Commit (with the resolved new selected index);
/// `on_cancelled` → Cancel. `CustomValue` commits are dropped per
/// FORBID-PE-AUTO-MATCH-AT-COMMIT (§9.2): PE never substitutes a typed
/// value for an option.
///
/// SPEC-HAL-260A §6.3 primary-action-ready contract: calls
/// `ComboBox::open_popup()` before returning so the popup is visible
/// from the moment the editor mounts. The user pays one click for
/// double-click edit intent; they should not pay another click to
/// open the dropdown.
fn make_combo_box(
    selected: usize,
    options: Vec<String>,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> ComboBox {
    let resolver = ResolverHandle::new(DefaultSelectionResolver::strict());
    let mut cb = ComboBox::new(resolver);
    let opts: Vec<(OptionId, &str)> = options
        .iter()
        .enumerate()
        .map(|(i, s)| (OptionId::from_raw(i as u64), s.as_str()))
        .collect();
    cb.set_options(&opts);
    if selected < options.len() {
        cb.set_input_text(options[selected].clone())
            .expect("new ComboBox accepts construction props before mounting");
    }
    let options_for_commit = options;
    let on_exit_commit = on_exit.clone();
    cb.set_on_committed(move |committed: &CommittedValue| {
        if let CommittedValue::ExistingOption(opt_id) = committed {
            let new_selected = opt_id.raw() as usize;
            let new_value = PropertyValue::Choice {
                selected: new_selected,
                options: options_for_commit.clone(),
            };
            on_exit_commit(EditExitReason::Commit, Some(new_value));
        }
        // CustomValue is dropped — FORBID-PE-AUTO-MATCH-AT-COMMIT (§9.2).
    });
    let on_exit_cancel = on_exit;
    cb.set_on_cancelled(move || {
        on_exit_cancel(EditExitReason::Cancel, None);
    });

    // SPEC-HAL-260A §6.3 primary-action-ready contract: PE editors mount
    // in response to a definitive edit-intent gesture (double-click on a
    // Data cell, or `i`/Enter on a focused cell). After that gesture,
    // the editor must land in its primary-action state — for ComboBox,
    // popup open and ready to select. `ComboBox::open_popup` is the
    // public API authored for exactly this call site (see its doc
    // comment); it sets `focused`, `in_capture`, and `popup_open` without
    // requesting framework keyboard capture, so the framework's
    // KeyboardAuthorityMode stays in Navigation at mount time.
    cb.open_popup();

    cb
}

/// ColorPickerField for `PropertyValue::Color`. Wires the ColorPickerField
/// callbacks: `on_committed` (OK button) → Commit; `on_cancelled` (Cancel
/// button / outside-click / Escape) → Cancel.
fn make_color_picker_field(
    initial: Color,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> ColorPickerField {
    let mut field = ColorPickerField::new().with_color(initial);
    let on_exit_commit = on_exit.clone();
    field.set_on_committed(move |c: Color| {
        on_exit_commit(EditExitReason::Commit, Some(PropertyValue::Color(c)));
    });
    let on_exit_cancel = on_exit;
    field.set_on_cancelled(move || {
        on_exit_cancel(EditExitReason::Cancel, None);
    });
    field
}

/// KeybindEditor for `PropertyValue::KeyBinding`. Wires `on_change`:
/// `Some(Shortcut)` → Commit; `None` (Backspace-clear with no modifiers)
/// → Cancel. Escape during capture does NOT fire `on_change` (widget's
/// `on_capture_leave` per SPEC-HAL-329 §5.3 / INV-KBE-NO-OVERRIDE-ESCAPE);
/// PE Body detects capture-leave on this widget and emits Cancel
/// externally (Phase B concern).
fn make_keybind_editor(
    initial: Option<Shortcut>,
    _cell_key: (u64, ColumnId),
    on_exit: Rc<dyn Fn(EditExitReason, Option<PropertyValue>)>,
) -> KeybindEditor {
    // JC directive 2026-05-21 (steal-binding): both Some and None branches
    // route to Commit. Some = new shortcut captured; None = Backspace
    // cleared the binding (INV-KBE-BACKSPACE-CLEARS). The cell's value
    // becomes KeyBinding(new_value) verbatim. The data source decides
    // what to do — the demo's commit_cell handles steal-from-previous-
    // owner and registry updates.
    //
    // on_cancel routes framework-initiated capture release without a
    // commit (Escape / Tab / blur during capture) to EditExitReason::Cancel.
    // on_change and on_cancel are mutually exclusive per session per
    // the widget's committed_in_session guard.
    let on_exit_commit = on_exit.clone();
    let on_exit_cancel = on_exit;
    KeybindEditor::new()
        .with_value(initial)
        .on_change(move |new_value: Option<Shortcut>| {
            on_exit_commit(
                EditExitReason::Commit,
                Some(PropertyValue::KeyBinding(new_value)),
            );
        })
        .on_cancel(move || {
            on_exit_cancel(EditExitReason::Cancel, None);
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::{Cell, RefCell};
    use std::time::Instant;

    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::event::{
        FusedInputEvent, InteractionDomainState, KeyCode, Modifiers, PhysicalKeyCode, WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::icon::IconRegistry;
    use crate::core::layout::Constraints;
    use crate::core::math::Size;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::runtime::actions::ActionId;
    use crate::runtime::binding_registry::BindingRegistry;

    #[test]
    fn composite_editor_factory_commit_uses_current_mapping_and_one_callback_slot() {
        let values = Rc::new(RefCell::new(Vec::new()));
        let observed = Rc::clone(&values);
        let factory = DefaultCellEditorFactory::new();
        let widget = factory.create_combo_box_editor(
            &ComboBoxCellEditorSpec {
                current_value: "Same".into(),
                options: vec![("Same".into(), "old".into())],
            },
            (1, ColumnId::new()),
            Box::new(move |value| observed.borrow_mut().push(value)),
        );
        let mut tree = WidgetTree::new();
        let root = tree.create_root(widget, None, 0);
        let owner = tree.node(root).unwrap().id();
        tree.node_mut(root)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ComboBox>()
            .unwrap()
            .set_options_with_values(&[(OptionId::from_raw(0), "Same", "new")]);

        let mut engine = FontManagerTextEngine::new();
        let mut scene = RetainedSceneTestHarness::new();
        scene.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(240.0, 40.0)),
            &Theme::dark(),
            &mut engine,
            &IconRegistry::new(&[]),
        );
        let mut router = EventRouter::new();
        let mut domain = InteractionDomainState::new();
        let mut bindings = BindingRegistry::new();
        router.request_focus_with_text_engine(&mut tree, owner, &mut domain, Some(&mut engine));

        // Removing the highlighted option must preserve the resolver's refusal:
        // no stale initial mapping, child reset, callback, or Capture release.
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            &mut domain,
            &mut engine,
            None,
        ));
        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Down,
                physical_key: PhysicalKeyCode::ArrowDown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut engine,
            None,
            &mut domain,
            &mut bindings,
        );
        let editor = tree.composite_editor_id(owner).unwrap();
        let before_rejection = tree.composite_editor_snapshot(&editor).unwrap();
        tree.node_mut(root)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ComboBox>()
            .unwrap()
            .set_options_with_values(&[]);
        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Enter,
                physical_key: PhysicalKeyCode::Enter,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut engine,
            None,
            &mut domain,
            &mut bindings,
        );
        assert_eq!(
            tree.composite_editor_snapshot(&editor).unwrap(),
            before_rejection
        );
        assert!(values.borrow().is_empty());
        assert_eq!(
            domain.keyboard_mode,
            crate::core::event::KeyboardAuthorityMode::Capture
        );
        assert_eq!(domain.focus_manager.focused(), Some(owner));
        let combo = tree
            .node_mut(root)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ComboBox>()
            .unwrap();
        assert!(combo.take_committed().is_none());
        combo.set_options_with_values(&[(OptionId::from_raw(0), "Same", "new")]);

        let public_calls = Rc::new(Cell::new(0));
        for commit in 0..2 {
            if commit == 1 {
                let calls = Rc::clone(&public_calls);
                tree.node_mut(root)
                    .unwrap()
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<ComboBox>()
                    .unwrap()
                    .set_on_committed(move |_| calls.set(calls.get() + 1));
            }
            if domain.keyboard_mode != crate::core::event::KeyboardAuthorityMode::Capture {
                assert!(router.handle_engine_action(
                    &mut tree,
                    ActionId::ENTER_WIDGET,
                    Some(root),
                    &mut domain,
                    &mut engine,
                    None,
                ));
            }
            for (key_code, physical_key) in [
                (KeyCode::Down, PhysicalKeyCode::ArrowDown),
                (KeyCode::Enter, PhysicalKeyCode::Enter),
            ] {
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::KeyDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        key_code,
                        physical_key,
                        reservation: None,
                        modifiers: Modifiers::default(),
                        repeat: false,
                    },
                    Some(root),
                    &mut engine,
                    None,
                    &mut domain,
                    &mut bindings,
                );
            }
        }
        assert_eq!(&*values.borrow(), &["new".to_owned()]);
        assert_eq!(public_calls.get(), 1);
    }
}
