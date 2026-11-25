// =============================================================================
// SPEC-HAL-200: Tabular Intent Policy
// =============================================================================
//
// Substrate-level, declarative intent policy that governs interaction
// semantics for all tabular widgets. The policy interprets raw input events
// (pointer, keyboard, modifiers) into semantic intents. It does not own
// state, mutate data, or depend on geometry.
//
// =============================================================================
// BINDING INVARIANTS
// =============================================================================
//
// INV-INTENT-STATELESS-1: Policy contains configuration fields only.
//   No runtime state: no selection, no focus, no anchor, no hover, no drag.
//
// INV-INTENT-PURE-1: Policy methods are pure functions.
//   Read input and context, return intents, mutate nothing.
//
// INV-INTENT-SEMANTIC-1: Policy operates on semantic context only.
//   Never receives or inspects geometry, pixel coordinates, widget rects,
//   or scroll offsets.
//
// INV-INTENT-OWNERSHIP-1: All interaction state is owned externally.
//   Selection, focus, anchor, hover, drag are owned by substrate or widget.
//
// INV-INTENT-IDENTITY-1: Policy reasons about RowId, never about indices.
//
// INV-INTENT-SINGLE-1: Exactly one TabularIntentPolicy per tabular consumer.
//   No per-layer shadow policy. No per-event overrides.
//
// =============================================================================

use super::intents::{ActivationIntent, ExpansionIntent, SelectionIntent, TabularIntent};
use super::row::RowId;
use crate::core::event::{KeyCode, Modifiers};
use crate::core::ordered_selection::{SelectionInteractionModel, SelectionMode};

// =============================================================================
// Policy Configuration Enums
// =============================================================================

/// When activation callbacks fire.
///
/// SPEC-HAL-200 §7.2: Activation is distinct from selection.
/// Activation opens, runs, or invokes. Selection marks for batch operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActivationMode {
    /// Activate on single click (typical for single-selection lists).
    OnClick,
    /// Activate on double-click (typical for multi-selection lists).
    OnDoubleClick,
    /// Activation only via explicit keyboard (Enter) or programmatic call.
    ExplicitOnly,
}

/// Whether Shift+click/Shift+nav performs range selection.
///
/// SPEC-HAL-200 §7.3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangePolicy {
    /// Shift has no special behavior.
    Disabled,
    /// Shift+click/Shift+nav selects all rows from anchor to target.
    ShiftRange,
}

/// Whether pointer drag performs range selection.
///
/// SPEC-HAL-200 §7.4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DragPolicy {
    /// Drag has no selection effect.
    Disabled,
    /// Drag from press point selects range continuously.
    RangeOnDrag,
}

/// Whether clicking a subcontrol selects the parent row.
///
/// SPEC-HAL-200 §7.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubcontrolBehavior {
    /// Clicking anywhere in the row (including subcontrols) selects it.
    ClickSelectsRow,
    /// Clicking subcontrols does not select the row.
    ClickDoesNotSelectRow,
}

// =============================================================================
// Intent Input and Context
// =============================================================================

/// Semantic input abstraction.
///
/// SPEC-HAL-200 §8.1: Widgets translate raw PointerEvent/KeyEvent into this
/// before consulting the policy. Widgets are responsible for hit-testing
/// coordinates to RowId, detecting click vs double-click vs drag, and
/// classifying subcontrol hits. Widgets are NOT responsible for interpreting
/// what these inputs mean.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IntentInput {
    Click {
        target: RowId,
        modifiers: Modifiers,
        is_subcontrol: bool,
    },
    DoubleClick {
        target: RowId,
    },
    DragStart {
        target: RowId,
        modifiers: Modifiers,
    },
    DragMove {
        target: RowId,
    },
    DragEnd,
    KeyPress {
        key: KeyCode,
        modifiers: Modifiers,
    },
}

/// State snapshot provided at interpretation time.
///
/// SPEC-HAL-200 §8.2: Read-only context. The policy never writes to it.
/// Widgets/substrate populate this before calling interpret().
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IntentContext {
    /// Whether the widget is focused.
    pub focused: bool,
    /// Whether a selection anchor exists.
    pub has_anchor: bool,
    /// Whether inline editing is active.
    pub is_editing: bool,
    /// Current focused row, if any.
    pub focused_row: Option<RowId>,
    /// SPEC-HAL-313: Whether the currently focused row is expanded.
    /// Used by h/l key interpretation to decide fall-through behavior.
    pub focused_row_expanded: bool,
}

impl IntentContext {
    /// Default context for testing: unfocused, no anchor, not editing, no focused row.
    pub fn default_test() -> Self {
        Self {
            focused: false,
            has_anchor: false,
            is_editing: false,
            focused_row: None,
            focused_row_expanded: false,
        }
    }
}

// =============================================================================
// TabularIntentPolicy
// =============================================================================

/// Declarative intent policy for all tabular consumers.
///
/// SPEC-HAL-200 §7 / SPEC-HAL-347: All fields are configuration values set
/// before the first routed row interaction. None are mutated after the
/// interaction-model lock is active.
///
/// FORBID-INTENT-3: This struct MUST NOT contain Cell, RefCell, Rc, Arc,
/// or any interior mutability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TabularIntentPolicy {
    /// How many items may be selected simultaneously.
    pub selection_mode: SelectionMode,
    /// When activation callbacks fire.
    pub activation_mode: ActivationMode,
    /// Whether Shift+click/Shift+nav performs range selection.
    pub range_selection: RangePolicy,
    /// Whether pointer drag performs range selection.
    pub drag_selection: DragPolicy,
    /// Whether clicking a subcontrol selects the parent row.
    pub subcontrol_behavior: SubcontrolBehavior,
    /// Immediate vs transactional selection model.
    pub interaction_model: SelectionInteractionModel,
    /// SPEC-HAL-313: Whether row expansion is enabled.
    /// When true, Enter/Space/h/l/i/double-click emit ExpansionIntent
    /// instead of their default semantics.
    pub expansion_enabled: bool,
}

impl TabularIntentPolicy {
    /// Single selection, click-to-activate, no range, no drag.
    pub fn single() -> Self {
        Self {
            selection_mode: SelectionMode::Single,
            activation_mode: ActivationMode::OnClick,
            range_selection: RangePolicy::Disabled,
            drag_selection: DragPolicy::Disabled,
            subcontrol_behavior: SubcontrolBehavior::ClickSelectsRow,
            interaction_model: SelectionInteractionModel::Immediate,
            expansion_enabled: false,
        }
    }

    /// Multi selection, double-click-to-activate, Shift range, range drag.
    pub fn multi() -> Self {
        Self {
            selection_mode: SelectionMode::Multi,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::ShiftRange,
            drag_selection: DragPolicy::RangeOnDrag,
            subcontrol_behavior: SubcontrolBehavior::ClickDoesNotSelectRow,
            interaction_model: SelectionInteractionModel::Immediate,
            expansion_enabled: false,
        }
    }

    /// No selection allowed (focus-only navigation).
    pub fn focus_only() -> Self {
        Self {
            selection_mode: SelectionMode::None,
            activation_mode: ActivationMode::ExplicitOnly,
            range_selection: RangePolicy::Disabled,
            drag_selection: DragPolicy::Disabled,
            subcontrol_behavior: SubcontrolBehavior::ClickSelectsRow,
            interaction_model: SelectionInteractionModel::Immediate,
            expansion_enabled: false,
        }
    }

    /// Interpret a semantic input event into zero or more tabular intents.
    ///
    /// SPEC-HAL-200 §8: Single interpretation entry point.
    /// Returns a Vec<TabularIntent>; a single input may produce multiple intents.
    ///
    /// INV-INTENT-PURE-1: This method is a pure function. It reads input and
    /// context, returns intents, and mutates nothing.
    pub fn interpret(&self, event: &IntentInput, context: &IntentContext) -> Vec<TabularIntent> {
        match event {
            IntentInput::Click {
                target,
                modifiers,
                is_subcontrol,
            } => self.interpret_click(*target, *modifiers, *is_subcontrol),
            IntentInput::DoubleClick { target } => self.interpret_double_click(*target),
            IntentInput::DragStart { .. } => {
                // SPEC-HAL-200 §8.5: Drag state (active, anchor) is owned by the widget.
                // DragStart does not produce selection intents from the policy.
                Vec::new()
            }
            IntentInput::DragMove { target } => self.interpret_drag_move(*target),
            IntentInput::DragEnd => Vec::new(),
            IntentInput::KeyPress { key, modifiers } => {
                self.interpret_key_press(*key, *modifiers, context)
            }
        }
    }

    // =========================================================================
    // Pointer interpretation — SPEC-HAL-200 §8.3
    // =========================================================================

    fn interpret_click(
        &self,
        target: RowId,
        modifiers: Modifiers,
        is_subcontrol: bool,
    ) -> Vec<TabularIntent> {
        // §8.3 Subcontrol Click
        if is_subcontrol && self.subcontrol_behavior == SubcontrolBehavior::ClickDoesNotSelectRow {
            return Vec::new();
        }

        // SelectionMode::None → no selection intents
        if self.selection_mode == SelectionMode::None {
            return Vec::new();
        }

        let is_ctrl_or_cmd = modifiers.ctrl || modifiers.meta;
        let is_shift = modifiers.shift;

        let mut intents = Vec::new();

        if is_ctrl_or_cmd {
            // §8.3 Ctrl/Cmd+Click
            intents.push(TabularIntent::Selection(SelectionIntent::ToggleRow {
                row_id: target,
            }));
        } else if is_shift {
            // §8.3 Shift+Click
            match (self.range_selection, self.selection_mode) {
                (RangePolicy::Disabled, _) => {
                    // Same as plain click
                    intents.push(TabularIntent::Selection(SelectionIntent::SelectRow {
                        row_id: target,
                    }));
                }
                (RangePolicy::ShiftRange, SelectionMode::None) => {
                    // No effect (already handled above, but defensive)
                }
                (RangePolicy::ShiftRange, SelectionMode::Single) => {
                    // Range not meaningful for single
                    intents.push(TabularIntent::Selection(SelectionIntent::SelectRow {
                        row_id: target,
                    }));
                }
                (RangePolicy::ShiftRange, SelectionMode::Multi) => {
                    intents.push(TabularIntent::Selection(SelectionIntent::ExtendToRow {
                        row_id: target,
                    }));
                }
            }
        } else {
            // §8.3 Plain Click (no modifiers)
            intents.push(TabularIntent::Selection(SelectionIntent::SelectRow {
                row_id: target,
            }));

            // Activation on plain click only
            if self.activation_mode == ActivationMode::OnClick {
                intents.push(TabularIntent::Activation(ActivationIntent::Activate {
                    row_id: target,
                }));
            }
        }

        intents
    }

    fn interpret_double_click(&self, target: RowId) -> Vec<TabularIntent> {
        // SPEC-HAL-313: Expansion takes priority over activation on double-click.
        if self.expansion_enabled {
            vec![TabularIntent::Expansion(ExpansionIntent::Toggle {
                row_id: target,
            })]
        } else if self.activation_mode == ActivationMode::OnDoubleClick {
            vec![TabularIntent::Activation(ActivationIntent::Activate {
                row_id: target,
            })]
        } else {
            Vec::new()
        }
    }

    // =========================================================================
    // Drag interpretation — SPEC-HAL-200 §8.5
    // =========================================================================

    fn interpret_drag_move(&self, target: RowId) -> Vec<TabularIntent> {
        match self.drag_selection {
            DragPolicy::Disabled => Vec::new(),
            DragPolicy::RangeOnDrag => {
                vec![TabularIntent::Selection(SelectionIntent::ExtendToRow {
                    row_id: target,
                })]
            }
        }
    }

    // =========================================================================
    // Keyboard interpretation — SPEC-HAL-200 §8.4
    // =========================================================================

    fn interpret_key_press(
        &self,
        key: KeyCode,
        modifiers: Modifiers,
        context: &IntentContext,
    ) -> Vec<TabularIntent> {
        match key {
            // Navigation keys: Up/Down/Home/End/PageUp/PageDown, plus vim-style j/k.
            //
            // SPEC-HAL-313 §3.3: j/k are intra-list row navigation. They reach this
            // policy only in Capture mode — in Navigation mode the framework
            // (route_input_event hjkl→Arrow translation) rewrites them to Down/Up
            // before they reach the widget. Without these arms, Capture-mode j/k
            // would fall to the wildcard `_ => Vec::new()` below: handle_keyboard's
            // j/k arms would still compute target_index and call set_focus, but no
            // SelectionIntent would fire, leaving selection on the previous row
            // while focus advances.
            KeyCode::Up
            | KeyCode::Down
            | KeyCode::Home
            | KeyCode::End
            | KeyCode::PageUp
            | KeyCode::PageDown
            | KeyCode::Character('j')
            | KeyCode::Character('k') => self.interpret_navigation(modifiers, context),

            // Enter: expansion toggle (if enabled) or activation
            KeyCode::Enter => {
                if let Some(row_id) = context.focused_row {
                    if self.expansion_enabled {
                        vec![TabularIntent::Expansion(ExpansionIntent::Toggle { row_id })]
                    } else {
                        vec![TabularIntent::Activation(ActivationIntent::Activate {
                            row_id,
                        })]
                    }
                } else {
                    Vec::new()
                }
            }

            // Escape: unchanged
            KeyCode::Escape => {
                if context.is_editing {
                    Vec::new()
                } else {
                    vec![TabularIntent::Selection(SelectionIntent::ClearSelection)]
                }
            }

            // Space: expansion toggle (if enabled) or selection toggle
            KeyCode::Space => {
                if self.expansion_enabled {
                    if let Some(row_id) = context.focused_row {
                        vec![TabularIntent::Expansion(ExpansionIntent::Toggle { row_id })]
                    } else {
                        Vec::new()
                    }
                } else {
                    self.interpret_space(context)
                }
            }

            // SPEC-HAL-313 §3.3: h = collapse (if expanded), else fall through
            // INV-313-FALLTHROUGH-1: Empty vec → widget returns false → Action System
            KeyCode::Character('h') if self.expansion_enabled => {
                if let Some(row_id) = context.focused_row {
                    if context.focused_row_expanded {
                        vec![TabularIntent::Expansion(ExpansionIntent::Collapse {
                            row_id,
                        })]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }

            // SPEC-HAL-313 §3.3: l = expand (if collapsed), else fall through
            KeyCode::Character('l') if self.expansion_enabled => {
                if let Some(row_id) = context.focused_row {
                    if !context.focused_row_expanded {
                        vec![TabularIntent::Expansion(ExpansionIntent::Expand { row_id })]
                    } else {
                        Vec::new()
                    }
                } else {
                    Vec::new()
                }
            }

            // SPEC-HAL-313 §3.3: i = activate focused row.
            // In Navigation mode, `i` is consumed by ENTER_WIDGET (capture entry)
            // before reaching IntentPolicy. This arm fires only in Capture mode
            // (via synthetic KeyEvent from Event::Text handler).
            KeyCode::Character('i') if self.expansion_enabled => {
                if let Some(row_id) = context.focused_row {
                    vec![TabularIntent::Activation(ActivationIntent::Activate {
                        row_id,
                    })]
                } else {
                    Vec::new()
                }
            }

            // All other keys: no intent from the policy
            _ => Vec::new(),
        }
    }

    /// Navigation keys (Up/Down/Home/End/PageUp/PageDown).
    ///
    /// Convention: The widget resolves the navigation target row before calling
    /// interpret(), and provides it as context.focused_row. The policy uses
    /// this pre-resolved row for selection intents.
    fn interpret_navigation(
        &self,
        modifiers: Modifiers,
        context: &IntentContext,
    ) -> Vec<TabularIntent> {
        if self.selection_mode == SelectionMode::None {
            return Vec::new();
        }

        let Some(row_id) = context.focused_row else {
            return Vec::new();
        };

        let is_ctrl_or_cmd = modifiers.ctrl || modifiers.meta;
        let is_shift = modifiers.shift;

        // §8.4: Ctrl/Cmd → Focus only (no selection)
        if is_ctrl_or_cmd {
            return Vec::new();
        }

        // §8.4: Shift + Multi + ShiftRange → ExtendToRow
        if is_shift
            && self.range_selection == RangePolicy::ShiftRange
            && self.selection_mode == SelectionMode::Multi
        {
            return vec![TabularIntent::Selection(SelectionIntent::ExtendToRow {
                row_id,
            })];
        }

        // §8.4: Plain or Shift+Single/Disabled → SelectRow
        vec![TabularIntent::Selection(SelectionIntent::SelectRow {
            row_id,
        })]
    }

    fn interpret_space(&self, context: &IntentContext) -> Vec<TabularIntent> {
        let Some(row_id) = context.focused_row else {
            return Vec::new();
        };

        match self.selection_mode {
            SelectionMode::None => Vec::new(),
            SelectionMode::Single => vec![TabularIntent::Selection(SelectionIntent::SelectRow {
                row_id,
            })],
            SelectionMode::Multi => vec![TabularIntent::Selection(SelectionIntent::ToggleRow {
                row_id,
            })],
        }
    }
}
