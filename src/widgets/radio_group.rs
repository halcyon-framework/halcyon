// SPEC-HAL-130 RadioGroup Core Reducer
// Pure semantic core. No widget. No paint. No input mapping.

// ============================================================================
// RadioId Type (SPEC-HAL-130 Stage 1)
// ============================================================================

/// Opaque identifier for a radio group member.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadioId(pub u32);

// ============================================================================
// Core State (SPEC-HAL-130 Stage 1)
// ============================================================================

/// RadioGroup state vector (SPEC-HAL-130 Stage 1)
///
/// Selection is group-owned, not member-owned.
/// Individual RadioButton members are stateless projections.
#[derive(Clone, Debug, PartialEq)]
pub struct RadioGroupState {
    /// Sole persistent selection value (INV-G1)
    pub selected: Option<RadioId>,
    /// Hard override for entire group (INV-G5)
    pub disabled: bool,
    /// Which member owns focus within the group (INV-G3)
    pub focused: Option<RadioId>,
    /// Which member is armed/pressed (INV-G4)
    pub active: Option<RadioId>,
}

impl Default for RadioGroupState {
    fn default() -> Self {
        Self {
            selected: None,
            disabled: false,
            focused: None,
            active: None,
        }
    }
}

// ============================================================================
// Semantic Events (SPEC-HAL-130 Stage 2) — CLOSED
// ============================================================================

/// Semantic events for RadioGroup (SPEC-HAL-130 Stage 2 + Stage 3)
///
/// This is the complete, closed set of events the group processes.
/// No other event type is legal input (INV-E1).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RadioGroupEvent {
    /// Request to select a member (direct or arrow-key navigation)
    Select(RadioId),
    /// Arm + commit selection (pointer release or keyboard activation)
    Activate(RadioId),
    /// Abort transient active state without changing selected
    Cancel(RadioId),
    /// Member gains focus within group
    FocusGain(RadioId),
    /// Member loses focus within group
    FocusLoss(RadioId),
    /// Stage 3: Arm a member (pointer down visual feedback)
    Press(RadioId),
}

// ============================================================================
// Core Reducer (SPEC-HAL-130 Stages 3 + 4)
// ============================================================================

/// Pure reducer implementing SPEC-HAL-130 Stages 3 + 4.
///
/// Mutates `state` synchronously.
/// Returns `Some(new_selected)` only when `selected` actually changes.
/// Returns `None` otherwise.
///
/// All transitions are atomic (INV-T2) and synchronous (INV-T4).
pub fn reduce_radio_group(
    state: &mut RadioGroupState,
    event: RadioGroupEvent,
) -> Option<Option<RadioId>> {
    // INV-G5: Disabled overrides all (except selected survives per INV-G6)
    if state.disabled {
        return None;
    }

    match event {
        RadioGroupEvent::Select(member) => {
            // Stage 3: Select sets selected = Some(member)
            // focused and active unchanged
            let old_selected = state.selected;
            state.selected = Some(member);

            // Stage 4: Emit only if selected actually changed
            if state.selected != old_selected {
                Some(state.selected)
            } else {
                None
            }
        }

        RadioGroupEvent::Activate(member) => {
            // Stage 3: Activate commits selection and clears active atomically
            let old_selected = state.selected;
            state.selected = Some(member);
            state.active = None;

            // Stage 4: Emit only if selected actually changed
            if state.selected != old_selected {
                Some(state.selected)
            } else {
                None
            }
        }

        RadioGroupEvent::Cancel(member) => {
            // Stage 3: Cancel clears active only if active = Some(member)
            // Precondition: active = Some(member)
            if state.active == Some(member) {
                state.active = None;
            }
            // Never emits (selected unchanged)
            None
        }

        RadioGroupEvent::FocusGain(member) => {
            // Stage 3: FocusGain sets focused = Some(member)
            // Previous focus holder loses focus implicitly (mutual exclusion by assignment)
            state.focused = Some(member);
            // Never emits (selected unchanged)
            None
        }

        RadioGroupEvent::FocusLoss(member) => {
            // Stage 3: FocusLoss clears focused only if focused = Some(member)
            // Precondition: focused = Some(member)
            if state.focused == Some(member) {
                state.focused = None;
                // If the losing member was armed, active is cleared
                if state.active == Some(member) {
                    state.active = None;
                }
            }
            // Never emits (selected unchanged)
            None
        }

        RadioGroupEvent::Press(member) => {
            // Stage 3: Press arms a member (visual feedback on pointer down)
            // Sets active = Some(member), does not commit selection
            state.active = Some(member);
            // Never emits (selected unchanged)
            None
        }
    }
}
