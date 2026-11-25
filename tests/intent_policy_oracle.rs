// SPEC-HAL-200: TabularIntentPolicy Test Oracle
//
// Pure unit tests for TabularIntentPolicy::interpret().
// No widgets, substrate, renderer, or geometry.
// Covers the minimum test matrix from SPEC-HAL-200 §13.

use halcyon::core::event::{KeyCode, Modifiers};
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::widgets::tabular_view::{
    ActivationIntent, ActivationMode, DragPolicy, IntentContext, IntentInput, RangePolicy, RowId,
    SelectionIntent, SubcontrolBehavior, TabularIntent, TabularIntentPolicy,
};

// ============================================================================
// Helpers
// ============================================================================

fn row(id: u64) -> RowId {
    RowId::from_raw(id)
}

fn no_modifiers() -> Modifiers {
    Modifiers::default()
}

fn ctrl() -> Modifiers {
    Modifiers {
        ctrl: true,
        ..Default::default()
    }
}

fn cmd() -> Modifiers {
    Modifiers {
        meta: true,
        ..Default::default()
    }
}

fn shift() -> Modifiers {
    Modifiers {
        shift: true,
        ..Default::default()
    }
}

fn ctx_default() -> IntentContext {
    IntentContext::default_test()
}

fn ctx_with_focus(row_id: RowId) -> IntentContext {
    IntentContext {
        focused: true,
        has_anchor: false,
        is_editing: false,
        focused_row: Some(row_id),
        focused_row_expanded: false,
    }
}

fn ctx_editing(row_id: RowId) -> IntentContext {
    IntentContext {
        focused: true,
        has_anchor: false,
        is_editing: true,
        focused_row: Some(row_id),
        focused_row_expanded: false,
    }
}

/// Assert the output contains exactly the given selection intent.
fn assert_has_selection(intents: &[TabularIntent], expected: SelectionIntent) {
    let found = intents
        .iter()
        .any(|i| matches!(i, TabularIntent::Selection(s) if *s == expected));
    assert!(found, "Expected {:?} in {:?}", expected, intents);
}

/// Assert the output contains exactly the given activation intent.
fn assert_has_activation(intents: &[TabularIntent], expected: ActivationIntent) {
    let found = intents
        .iter()
        .any(|i| matches!(i, TabularIntent::Activation(a) if *a == expected));
    assert!(found, "Expected {:?} in {:?}", expected, intents);
}

/// Assert the output contains no selection intents.
fn assert_no_selection(intents: &[TabularIntent]) {
    let found = intents
        .iter()
        .any(|i| matches!(i, TabularIntent::Selection(_)));
    assert!(!found, "Expected no selection intents, got {:?}", intents);
}

/// Assert the output contains no activation intents.
fn assert_no_activation(intents: &[TabularIntent]) {
    let found = intents
        .iter()
        .any(|i| matches!(i, TabularIntent::Activation(_)));
    assert!(!found, "Expected no activation intents, got {:?}", intents);
}

// ============================================================================
// §13 Test Matrix — Pointer Events
// ============================================================================

/// Click | Single | None | SelectRow + Activate (if OnClick)
#[test]
fn click_single_no_modifiers() {
    let policy = TabularIntentPolicy::single();
    let r = row(1);
    let input = IntentInput::Click {
        target: r,
        modifiers: no_modifiers(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
    assert_has_activation(&result, ActivationIntent::Activate { row_id: r });
    assert_eq!(result.len(), 2);
}

/// Click | Multi | None | SelectRow (no activation — OnDoubleClick mode)
#[test]
fn click_multi_no_modifiers() {
    let policy = TabularIntentPolicy::multi();
    let r = row(2);
    let input = IntentInput::Click {
        target: r,
        modifiers: no_modifiers(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
    assert_no_activation(&result);
    assert_eq!(result.len(), 1);
}

/// Click | Multi | Ctrl | ToggleRow
#[test]
fn click_multi_ctrl() {
    let policy = TabularIntentPolicy::multi();
    let r = row(3);
    let input = IntentInput::Click {
        target: r,
        modifiers: ctrl(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::ToggleRow { row_id: r });
    assert_no_activation(&result);
    assert_eq!(result.len(), 1);
}

/// Click | Multi | Cmd (macOS) | ToggleRow
#[test]
fn click_multi_cmd() {
    let policy = TabularIntentPolicy::multi();
    let r = row(3);
    let input = IntentInput::Click {
        target: r,
        modifiers: cmd(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::ToggleRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// Click | Multi | Shift | ExtendToRow
#[test]
fn click_multi_shift() {
    let policy = TabularIntentPolicy::multi();
    let r = row(4);
    let input = IntentInput::Click {
        target: r,
        modifiers: shift(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::ExtendToRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// Click | None | Any | Focus only (empty intents)
#[test]
fn click_none_any_modifiers() {
    let policy = TabularIntentPolicy::focus_only();
    let r = row(5);

    for mods in [no_modifiers(), ctrl(), shift(), cmd()] {
        let input = IntentInput::Click {
            target: r,
            modifiers: mods,
            is_subcontrol: false,
        };
        let result = policy.interpret(&input, &ctx_default());
        assert!(
            result.is_empty(),
            "Expected empty for SelectionMode::None, got {:?} with {:?}",
            result,
            mods
        );
    }
}

/// DoubleClick | Multi | None | Activate (if OnDoubleClick)
#[test]
fn double_click_multi_activates() {
    let policy = TabularIntentPolicy::multi();
    let r = row(6);
    let input = IntentInput::DoubleClick { target: r };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_activation(&result, ActivationIntent::Activate { row_id: r });
    assert_eq!(result.len(), 1);
}

/// DoubleClick | Single (OnClick activation) | None | No activation from double-click
#[test]
fn double_click_single_no_activation() {
    let policy = TabularIntentPolicy::single();
    let r = row(7);
    let input = IntentInput::DoubleClick { target: r };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

// ============================================================================
// §13 Test Matrix — Keyboard Events
// ============================================================================

/// KeyPress(Down) | Single | None | SelectRow (next)
#[test]
fn key_down_single_no_modifiers() {
    let policy = TabularIntentPolicy::single();
    let next = row(10);
    let ctx = ctx_with_focus(next);
    let input = IntentInput::KeyPress {
        key: KeyCode::Down,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: next });
    assert_eq!(result.len(), 1);
}

/// KeyPress(Down) | Multi | Shift | ExtendToRow (next)
#[test]
fn key_down_multi_shift() {
    let policy = TabularIntentPolicy::multi();
    let next = row(11);
    let ctx = ctx_with_focus(next);
    let input = IntentInput::KeyPress {
        key: KeyCode::Down,
        modifiers: shift(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::ExtendToRow { row_id: next });
    assert_eq!(result.len(), 1);
}

/// KeyPress(Down) | Any | Ctrl | Focus only (empty)
#[test]
fn key_down_ctrl_focus_only() {
    let policy = TabularIntentPolicy::single();
    let next = row(12);
    let ctx = ctx_with_focus(next);
    let input = IntentInput::KeyPress {
        key: KeyCode::Down,
        modifiers: ctrl(),
    };
    let result = policy.interpret(&input, &ctx);

    assert!(result.is_empty());
}

/// KeyPress(Enter) | Any | None | Activate
#[test]
fn key_enter_activates() {
    let policy = TabularIntentPolicy::multi();
    let r = row(13);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Enter,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_activation(&result, ActivationIntent::Activate { row_id: r });
    assert_eq!(result.len(), 1);
}

/// KeyPress(Enter) with no focused row → empty
#[test]
fn key_enter_no_focus() {
    let policy = TabularIntentPolicy::single();
    let input = IntentInput::KeyPress {
        key: KeyCode::Enter,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

/// KeyPress(Esc) | Any | None | ClearSelection
#[test]
fn key_escape_clears_selection() {
    let policy = TabularIntentPolicy::multi();
    let input = IntentInput::KeyPress {
        key: KeyCode::Escape,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::ClearSelection);
    assert_eq!(result.len(), 1);
}

/// KeyPress(Esc) while editing → empty (widget handles edit cancel)
#[test]
fn key_escape_while_editing() {
    let policy = TabularIntentPolicy::multi();
    let r = row(14);
    let ctx = ctx_editing(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Escape,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert!(result.is_empty());
}

/// KeyPress(Space) | Multi | None | ToggleRow
#[test]
fn key_space_multi_toggles() {
    let policy = TabularIntentPolicy::multi();
    let r = row(15);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Space,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::ToggleRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// KeyPress(Space) | Single | None | SelectRow
#[test]
fn key_space_single_selects() {
    let policy = TabularIntentPolicy::single();
    let r = row(16);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Space,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// KeyPress(Space) | None | None | empty
#[test]
fn key_space_none_mode() {
    let policy = TabularIntentPolicy::focus_only();
    let r = row(17);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Space,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert!(result.is_empty());
}

// ============================================================================
// §13 Test Matrix — Drag Events
// ============================================================================

/// DragMove | Multi | None | ExtendToRow (if RangeOnDrag)
#[test]
fn drag_move_range_on_drag() {
    let policy = TabularIntentPolicy::multi();
    let r = row(20);
    let input = IntentInput::DragMove { target: r };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::ExtendToRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// DragMove | Multi | None | empty (if DragPolicy::Disabled)
#[test]
fn drag_move_disabled() {
    let mut policy = TabularIntentPolicy::multi();
    policy.drag_selection = DragPolicy::Disabled;
    let r = row(21);
    let input = IntentInput::DragMove { target: r };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

/// DragStart produces no intents (widget owns drag state)
#[test]
fn drag_start_empty() {
    let policy = TabularIntentPolicy::multi();
    let r = row(22);
    let input = IntentInput::DragStart {
        target: r,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

/// DragEnd produces no intents
#[test]
fn drag_end_empty() {
    let policy = TabularIntentPolicy::multi();
    let input = IntentInput::DragEnd;
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

// ============================================================================
// Additional Coverage — Edge Cases
// ============================================================================

/// Subcontrol click with ClickDoesNotSelectRow suppresses all intents
#[test]
fn subcontrol_click_does_not_select() {
    let policy = TabularIntentPolicy::multi();
    let r = row(30);
    let input = IntentInput::Click {
        target: r,
        modifiers: no_modifiers(),
        is_subcontrol: true,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

/// Subcontrol click with ClickSelectsRow behaves as normal click
#[test]
fn subcontrol_click_selects_row() {
    let policy = TabularIntentPolicy::single();
    let r = row(31);
    let input = IntentInput::Click {
        target: r,
        modifiers: no_modifiers(),
        is_subcontrol: true,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
    assert_has_activation(&result, ActivationIntent::Activate { row_id: r });
}

/// Shift+Click with RangePolicy::Disabled falls back to plain click
#[test]
fn shift_click_range_disabled() {
    let policy = TabularIntentPolicy::single();
    let r = row(32);
    let input = IntentInput::Click {
        target: r,
        modifiers: shift(),
        is_subcontrol: false,
    };
    let result = policy.interpret(&input, &ctx_default());

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
    assert_eq!(result.len(), 1);
}

/// Navigation with no focused row produces empty
#[test]
fn nav_no_focused_row() {
    let policy = TabularIntentPolicy::single();
    let input = IntentInput::KeyPress {
        key: KeyCode::Up,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx_default());

    assert!(result.is_empty());
}

/// Navigation with SelectionMode::None produces empty
#[test]
fn nav_none_mode() {
    let policy = TabularIntentPolicy::focus_only();
    let r = row(33);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Down,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert!(result.is_empty());
}

/// Home key with single mode selects
#[test]
fn key_home_single() {
    let policy = TabularIntentPolicy::single();
    let r = row(34);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::Home,
        modifiers: no_modifiers(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::SelectRow { row_id: r });
}

/// PageDown with Shift+Multi extends
#[test]
fn key_pagedown_multi_shift() {
    let policy = TabularIntentPolicy::multi();
    let r = row(35);
    let ctx = ctx_with_focus(r);
    let input = IntentInput::KeyPress {
        key: KeyCode::PageDown,
        modifiers: shift(),
    };
    let result = policy.interpret(&input, &ctx);

    assert_has_selection(&result, SelectionIntent::ExtendToRow { row_id: r });
}

// ============================================================================
// Factory Method Tests
// ============================================================================

#[test]
fn factory_single() {
    let p = TabularIntentPolicy::single();
    assert_eq!(p.selection_mode, SelectionMode::Single);
    assert_eq!(p.activation_mode, ActivationMode::OnClick);
    assert_eq!(p.range_selection, RangePolicy::Disabled);
    assert_eq!(p.drag_selection, DragPolicy::Disabled);
    assert_eq!(p.subcontrol_behavior, SubcontrolBehavior::ClickSelectsRow);
}

#[test]
fn factory_multi() {
    let p = TabularIntentPolicy::multi();
    assert_eq!(p.selection_mode, SelectionMode::Multi);
    assert_eq!(p.activation_mode, ActivationMode::OnDoubleClick);
    assert_eq!(p.range_selection, RangePolicy::ShiftRange);
    assert_eq!(p.drag_selection, DragPolicy::RangeOnDrag);
    assert_eq!(
        p.subcontrol_behavior,
        SubcontrolBehavior::ClickDoesNotSelectRow
    );
}

#[test]
fn factory_focus_only() {
    let p = TabularIntentPolicy::focus_only();
    assert_eq!(p.selection_mode, SelectionMode::None);
    assert_eq!(p.activation_mode, ActivationMode::ExplicitOnly);
}

// ============================================================================
// INV-INTENT-STATELESS-1: Policy is Copy, no interior mutability
// ============================================================================

#[test]
fn policy_is_copy() {
    let p1 = TabularIntentPolicy::single();
    let p2 = p1;
    assert_eq!(p1, p2);
}

// ============================================================================
// INV-INTENT-PURE-1: Same input + context → same output
// ============================================================================

#[test]
fn interpret_is_deterministic() {
    let policy = TabularIntentPolicy::multi();
    let r = row(50);
    let input = IntentInput::Click {
        target: r,
        modifiers: ctrl(),
        is_subcontrol: false,
    };
    let ctx = ctx_default();

    let r1 = policy.interpret(&input, &ctx);
    let r2 = policy.interpret(&input, &ctx);
    assert_eq!(r1, r2);
}
