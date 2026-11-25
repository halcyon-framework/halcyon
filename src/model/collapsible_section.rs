// SPEC-HAL-215: CollapsibleSection Controller
// Plain boolean controller for expanded/collapsed state.
// Not a widget. No implicit behavior. No side effects.

/// Explicit expanded/collapsed controller.
///
/// Owns exactly one boolean. Application code reads it to decide
/// whether content participates in layout. No implicit input handling,
/// no animation, no tree mutation.
pub struct CollapsibleSection {
    expanded: bool,
}

impl CollapsibleSection {
    pub fn new(expanded: bool) -> Self {
        Self { expanded }
    }

    pub fn is_expanded(&self) -> bool {
        self.expanded
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn toggle(&mut self) {
        self.expanded = !self.expanded;
    }
}
