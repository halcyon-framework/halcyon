// SPEC-HAL-329: KeybindEditor Demo
// SPEC-HAL-018: Uses App + Screen pattern
//
// SPEC-HAL-329 §6.5: demonstrates the displacement caller pattern — binding a
// shortcut already bound to another editor clears the prior binding. Shared
// `bindings` model is mutated in `on_change`; a `dirty` flag triggers rebuild;
// each editor mounts on rebuild with `.with_value(model[i])`, so the displaced
// editor visually clears on the next frame.
//
// Demo Law: handle_input returns false for all
// events. The demo does NOT consume Escape. The framework owns Escape for
// capture release and focus reset.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{FusedInputEvent, KeyCode};
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::runtime::actions::Shortcut;
use halcyon::widgets::{ConflictInfo, KeybindEditor};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

const NUM_EDITORS: usize = 3;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== KeybindEditor Demo ===");
    println!("Features:");
    println!("  - Single click any editor to enter capture");
    println!("  - In capture mode: press any key combination to bind");
    println!("  - Backspace (no modifiers) clears the binding");
    println!("  - Escape exits capture WITHOUT committing");
    println!("  - Tab in capture: framework moves focus, NO commit");
    println!("  - Displacement: binding a shortcut already held by another editor");
    println!("    clears that editor (single owner per shortcut)");
    println!("  - Editor #3: Ctrl+S triggers a synthetic external-conflict warning");
    println!("Press Escape (outside capture) to release focus; close window to exit.");
    println!("==========================\n");

    let app = App::new(AppConfig::new("KeybindEditor Demo").with_size(440.0, 320.0));
    app.run(KeybindEditorDemoScreen::new());
}

struct KeybindEditorDemoScreen {
    needs_rebuild: bool,
    placeholder: Label,
    /// Shared binding model. `bindings[i]` is the current value of editor i.
    bindings: Rc<RefCell<[Option<Shortcut>; NUM_EDITORS]>>,
    /// Set by an editor's on_change after the model is mutated; consumed by
    /// take_rebuild_flag to trigger a fresh tree build that picks up the new
    /// model state (including displaced editors).
    dirty: Rc<Cell<bool>>,
    commit_count: Rc<Cell<u32>>,
}

impl KeybindEditorDemoScreen {
    fn new() -> Self {
        let initial_2 = Shortcut::ctrl_shift(KeyCode::Character('S'));
        Self {
            needs_rebuild: true,
            placeholder: Label::new(""),
            bindings: Rc::new(RefCell::new([None, Some(initial_2), None])),
            dirty: Rc::new(Cell::new(false)),
            commit_count: Rc::new(Cell::new(0)),
        }
    }
}

/// SPEC-HAL-329 §6.5: build an on_change callback for editor index `idx`
/// implementing the displacement pattern. The closure:
/// 1. Scans the shared bindings model for any OTHER index holding the same
///    shortcut; clears each such entry.
/// 2. Writes the new value into `bindings[idx]`.
/// 3. Sets the dirty flag so the next frame rebuilds with the updated model.
fn make_on_change(
    idx: usize,
    bindings: Rc<RefCell<[Option<Shortcut>; NUM_EDITORS]>>,
    dirty: Rc<Cell<bool>>,
    commit_count: Rc<Cell<u32>>,
) -> impl Fn(Option<Shortcut>) + 'static {
    move |v| {
        let mut b = bindings.borrow_mut();
        if let Some(new) = v {
            for i in 0..b.len() {
                if i != idx && b[i] == Some(new) {
                    println!(
                        "[DISPLACE] Editor #{} cleared (was bound to {:?})",
                        i + 1,
                        b[i]
                    );
                    b[i] = None;
                }
            }
        }
        b[idx] = v;
        commit_count.set(commit_count.get() + 1);
        println!(
            "[CALLBACK #{}] bound = {:?} (total commits: {})",
            idx + 1,
            v,
            commit_count.get()
        );
        dirty.set(true);
    }
}

impl Screen for KeybindEditorDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let snapshot: [Option<Shortcut>; NUM_EDITORS] = *self.bindings.borrow();

        let root = cx.mount_child(Box::new(Flex::column().with_spacing(12.0)));

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(20.0)), None, 0);

        cx.tree
            .add_child(root, Box::new(Label::new("KeybindEditor Demo")), None, 1);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(8.0)), None, 2);

        let content = cx
            .tree
            .add_child(root, Box::new(Flex::column().with_spacing(10.0)), None, 3);

        // Editor #1.
        cx.tree
            .add_child(content, Box::new(Label::new("Editor #1:")), None, 0);
        let editor_1 = KeybindEditor::new()
            .with_placeholder("(no binding)")
            .with_value(snapshot[0])
            .on_change(make_on_change(
                0,
                self.bindings.clone(),
                self.dirty.clone(),
                self.commit_count.clone(),
            ));
        cx.tree.add_child(content, Box::new(editor_1), None, 1);

        cx.tree
            .add_child(content, Box::new(FixedSpacer::vertical(4.0)), None, 2);

        // Editor #2 — pre-bound (Ctrl+Shift+S).
        cx.tree
            .add_child(content, Box::new(Label::new("Editor #2:")), None, 3);
        let editor_2 = KeybindEditor::new()
            .with_placeholder("(no binding)")
            .with_value(snapshot[1])
            .on_change(make_on_change(
                1,
                self.bindings.clone(),
                self.dirty.clone(),
                self.commit_count.clone(),
            ));
        cx.tree.add_child(content, Box::new(editor_2), None, 4);

        cx.tree
            .add_child(content, Box::new(FixedSpacer::vertical(4.0)), None, 5);

        // Editor #3 — has external-conflict warning for Ctrl+S (synthetic
        // "Save" action). Displacement still applies for in-app conflicts.
        cx.tree.add_child(
            content,
            Box::new(Label::new(
                "Editor #3 (Ctrl+S → external 'Save' conflict warning):",
            )),
            None,
            6,
        );
        let editor_3 = KeybindEditor::new()
            .with_placeholder("(no binding)")
            .with_value(snapshot[2])
            .with_conflict_check(|s| {
                let ctrl_s_lower =
                    Shortcut::new(KeyCode::Character('s'), true, false, false, false);
                let ctrl_s_upper =
                    Shortcut::new(KeyCode::Character('S'), true, false, false, false);
                if s == ctrl_s_lower || s == ctrl_s_upper {
                    Some(ConflictInfo {
                        message: "Already bound to: Save (external)".into(),
                    })
                } else {
                    None
                }
            })
            .on_change(make_on_change(
                2,
                self.bindings.clone(),
                self.dirty.clone(),
                self.commit_count.clone(),
            ));
        cx.tree.add_child(content, Box::new(editor_3), None, 7);

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(16.0)), None, 4);

        cx.tree.add_child(
            root,
            Box::new(Label::new(
                "Click to capture | Backspace clears | Escape cancels | Same shortcut → displaces",
            )),
            None,
            5,
        );

        cx.tree
            .add_child(root, Box::new(FixedSpacer::vertical(12.0)), None, 6);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // DEMO LAW: demos do not consume any input.
        // The framework owns Escape (capture release / focus reset), Tab
        // (focus traversal), and all other framework keys.
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        // Combine the framework-driven rebuild flag with the demo's own
        // dirty flag set by editor on_change callbacks. Either one triggers
        // a rebuild that reads the current `bindings` snapshot.
        let dirty = self.dirty.replace(false);
        let needs = std::mem::take(&mut self.needs_rebuild);
        dirty || needs
    }
}
