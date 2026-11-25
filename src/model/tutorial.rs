// SPEC-HAL-227: Tutorial, Coachmark, and Guided Hint Sequencing
// Policy/orchestration layer only — no UI, no rendering, no timing.
//
// Binding invariants (mechanically true):
// INV-227-A1: No UI rendering
// INV-227-A2: No application action authority
// INV-227-A3: No implicit state discovery
// INV-227-B1: Explicit step graph only
// INV-227-B2: Single active step at a time
// INV-227-B3: Deterministic advancement only
// INV-227-C1: External signals only
// INV-227-C2: No timing authority
// INV-227-D1: Persistence is external and optional
// INV-227-D2: Stateless by default
// INV-227-E1: Anchoring delegated to SPEC-226

use crate::core::math::Point;
use crate::model::tooltip::TooltipPayload;

/// Identifies a tutorial step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TutorialStepId(pub u32);

/// A single step in a tutorial sequence.
/// Plain data — no callbacks, no behavior (INV-227-A2).
pub struct TutorialStep {
    pub id: TutorialStepId,
    pub payload: TooltipPayload,
    pub anchor: Point,
    /// Signal names that advance past this step (INV-227-C1).
    pub advance_signals: Vec<String>,
}

/// Explicit linear sequence of tutorial steps (INV-227-B1).
pub struct TutorialDefinition {
    steps: Vec<TutorialStep>,
}

impl TutorialDefinition {
    pub fn new(steps: Vec<TutorialStep>) -> Self {
        Self { steps }
    }
}

/// Command emitted by TutorialController.
/// Plain data consumed by application code to wire to TooltipSurface (INV-227-A1).
#[derive(Debug, Clone)]
pub enum TutorialCommand {
    Show {
        step_id: TutorialStepId,
        anchor: Point,
        payload: TooltipPayload,
    },
    Hide,
}

/// Pure state machine for tutorial sequencing (INV-227-B2, B3).
/// No IO, no timing, no widget references (INV-227-A1, A3, C2).
pub struct TutorialController {
    definition: TutorialDefinition,
    current_index: Option<usize>,
}

impl TutorialController {
    pub fn new(definition: TutorialDefinition) -> Self {
        Self {
            definition,
            current_index: None,
        }
    }

    /// Start the tutorial at the first step.
    /// Returns Show command for the first step, or None if definition is empty.
    pub fn start(&mut self) -> Option<TutorialCommand> {
        if self.definition.steps.is_empty() {
            return None;
        }
        self.current_index = Some(0);
        Some(self.show_current())
    }

    /// Send a named signal to the controller (INV-227-C1).
    /// If the signal matches the current step's advance_signals, advance.
    /// Returns the resulting command, or None if signal doesn't match.
    pub fn signal(&mut self, signal: &str) -> Option<TutorialCommand> {
        let index = self.current_index?;
        let step = &self.definition.steps[index];
        if !step.advance_signals.iter().any(|s| s == signal) {
            return None;
        }
        let next = index + 1;
        if next < self.definition.steps.len() {
            self.current_index = Some(next);
            Some(self.show_current())
        } else {
            self.current_index = None;
            Some(TutorialCommand::Hide)
        }
    }

    /// Dismiss the tutorial without completing it.
    /// Returns Hide command if active, None otherwise.
    pub fn dismiss(&mut self) -> Option<TutorialCommand> {
        if self.current_index.is_some() {
            self.current_index = None;
            Some(TutorialCommand::Hide)
        } else {
            None
        }
    }

    /// Current active step ID, if any (INV-227-B2).
    pub fn current_step_id(&self) -> Option<TutorialStepId> {
        self.current_index.map(|i| self.definition.steps[i].id)
    }

    /// Whether the tutorial is currently active.
    pub fn is_active(&self) -> bool {
        self.current_index.is_some()
    }

    fn show_current(&self) -> TutorialCommand {
        let step = &self.definition.steps[self.current_index.unwrap()];
        TutorialCommand::Show {
            step_id: step.id,
            anchor: step.anchor,
            payload: step.payload.clone(),
        }
    }
}
