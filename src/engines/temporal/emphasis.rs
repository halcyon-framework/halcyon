// =============================================================================
// SPEC-HAL-342: Temporal Emphasis — Shared Hover/Focus State
// =============================================================================
//
// A transient, identity-keyed emphasis state — the sibling of selection
// (SPEC-HAL-335 §3, INV-335-S1). A hovered focus id plus a resolved set of
// linked ids under a declared relation, exposed for projections to render.
// Selection persists; emphasis follows the pointer. The two compose, and
// neither reads nor writes the other (INV-342-E5).
//
// INV-342-E2: focus and linked set are IntervalIds (INV-335-ID1); never
//             positional (SPEC-PM-001).
// INV-342-E3: one hop only — Containment links the direct parent and direct
//             children, nothing transitive (SPEC-HAL-340 INV-340-C1). The
//             performance bound and the analytics boundary.
// INV-342-E4: the linked set is recomputed only when focus or relation changed
//             since the last resolve, and cached against them.
// INV-342-E7: Containment requires a ContainmentSource; absent one, resolution
//             degrades to Identity (empty linked set), never an error.
// =============================================================================

use crate::model::time::TimeRange;

use super::containment::ContainmentSource;
use super::descriptors::{IntervalId, TemporalDescriptors};

/// Which relation defines the linked set. SPEC-HAL-342 §3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisRelation {
    /// The focus alone, emphasized wherever it is drawn — across the panels and
    /// views that share the substrate. Linked set empty; needs no source.
    Identity,
    /// Focus plus its direct parent and direct children — one hop
    /// (SPEC-HAL-340 INV-340-C1). Requires a ContainmentSource; absent one,
    /// degrades to Identity (INV-342-E7).
    Containment,
    // Dependency — reserved. Not implemented in v1 (FORBID-342-4).
}

/// Emphasis role of an interval in the current frame. SPEC-HAL-342 §4.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisRole {
    None,
    Focus,
    Linked,
}

/// The hover/focus emphasis state — the transient sibling of
/// OrderedSelectionState.
#[derive(Debug, Default)]
pub struct EmphasisState {
    focus: Option<IntervalId>,
    linked: Vec<IntervalId>,
    // Gating cache (INV-342-E4): the (focus, relation) the linked set reflects.
    resolved_focus: Option<IntervalId>,
    resolved_relation: Option<EmphasisRelation>,
}

impl EmphasisState {
    pub fn new() -> Self {
        Self::default()
    }

    /// The current focus id, if any.
    pub fn focus(&self) -> Option<IntervalId> {
        self.focus
    }

    /// Set the hovered focus. Cheap; resolution is deferred to `resolve`
    /// (INV-342-E4).
    pub fn set_focus(&mut self, focus: Option<IntervalId>) {
        self.focus = focus;
    }

    /// Emphasis role of `id` in the current frame.
    pub fn role(&self, id: IntervalId) -> EmphasisRole {
        if self.focus == Some(id) {
            EmphasisRole::Focus
        } else if self.linked.contains(&id) {
            EmphasisRole::Linked
        } else {
            EmphasisRole::None
        }
    }

    /// Resolve the linked set for the current focus under `relation`, within
    /// `visible`. `containment` supplies the Containment relation's data; `None`
    /// degrades Containment to Identity (INV-342-E7). Recomputes only when focus
    /// or relation changed since the last resolve (INV-342-E4). One hop only
    /// (INV-342-E3).
    pub fn resolve(
        &mut self,
        relation: EmphasisRelation,
        containment: Option<&dyn ContainmentSource>,
        visible: TimeRange,
    ) {
        if self.resolved_focus == self.focus && self.resolved_relation == Some(relation) {
            return;
        }

        self.linked.clear();
        if let Some(focus) = self.focus {
            match relation {
                EmphasisRelation::Identity => {}
                EmphasisRelation::Containment => {
                    if let Some(src) = containment {
                        if let Some(parent) = src.parent_of(focus) {
                            self.linked.push(parent);
                        }
                        let mut children = TemporalDescriptors::default();
                        src.children_of(Some(focus), visible, &mut children);
                        for d in &children.intervals {
                            self.linked.push(d.id);
                        }
                    }
                }
            }
        }

        self.resolved_focus = self.focus;
        self.resolved_relation = Some(relation);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engines::temporal::containment::ContainmentPolicy;
    use crate::engines::temporal::descriptors::{IntervalDescriptor, TemporalInvalidationEvent};
    use crate::model::time::Timestamp;
    use std::cell::Cell;

    const FOCUS: IntervalId = IntervalId(10);
    const PARENT: IntervalId = IntervalId(1);

    /// Minimal containment source: a fixed parent for FOCUS, no children.
    /// Counts parent_of calls to verify resolve gating (INV-342-E4).
    struct Fixture {
        parent_calls: Cell<usize>,
    }

    impl Fixture {
        fn new() -> Self {
            Self {
                parent_calls: Cell::new(0),
            }
        }
    }

    impl ContainmentSource for Fixture {
        fn policy(&self) -> ContainmentPolicy {
            ContainmentPolicy::Enforced
        }
        fn children_of(
            &self,
            _parent: Option<IntervalId>,
            _range: TimeRange,
            _out: &mut TemporalDescriptors,
        ) {
        }
        fn parent_of(&self, id: IntervalId) -> Option<IntervalId> {
            self.parent_calls.set(self.parent_calls.get() + 1);
            (id == FOCUS).then_some(PARENT)
        }
        fn has_children(&self, _id: IntervalId) -> bool {
            false
        }
        fn interval(&self, _id: IntervalId) -> Option<IntervalDescriptor> {
            None
        }
        fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
            Vec::new()
        }
    }

    fn range() -> TimeRange {
        TimeRange::new(Timestamp(0), Timestamp(1_000))
    }

    #[test]
    fn identity_emphasizes_focus_only() {
        let mut e = EmphasisState::new();
        e.set_focus(Some(FOCUS));
        e.resolve(EmphasisRelation::Identity, None, range());
        assert_eq!(e.role(FOCUS), EmphasisRole::Focus);
        assert_eq!(e.role(PARENT), EmphasisRole::None);
    }

    #[test]
    fn containment_links_parent() {
        let fixture = Fixture::new();
        let mut e = EmphasisState::new();
        e.set_focus(Some(FOCUS));
        e.resolve(
            EmphasisRelation::Containment,
            Some(&fixture as &dyn ContainmentSource),
            range(),
        );
        assert_eq!(e.role(FOCUS), EmphasisRole::Focus);
        assert_eq!(e.role(PARENT), EmphasisRole::Linked);
    }

    #[test]
    fn containment_without_source_degrades_to_identity() {
        let mut e = EmphasisState::new();
        e.set_focus(Some(FOCUS));
        e.resolve(EmphasisRelation::Containment, None, range());
        assert_eq!(e.role(FOCUS), EmphasisRole::Focus);
        assert_eq!(e.role(PARENT), EmphasisRole::None);
    }

    #[test]
    fn resolve_is_gated_on_unchanged_focus() {
        let fixture = Fixture::new();
        let mut e = EmphasisState::new();
        e.set_focus(Some(FOCUS));
        let r = EmphasisRelation::Containment;
        e.resolve(r, Some(&fixture as &dyn ContainmentSource), range());
        e.resolve(r, Some(&fixture as &dyn ContainmentSource), range());
        assert_eq!(fixture.parent_calls.get(), 1);
    }
}
