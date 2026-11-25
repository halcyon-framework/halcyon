// =============================================================================
// SPEC-HAL-340: Temporal Containment — Parent/Child Structure
// =============================================================================
//
// A parent/child relation over identity-keyed intervals, with an enforced
// range-nesting discipline, consumed by drill-in / level-of-detail projections
// (SPEC-HAL-341). Substrate-level and reusable. Purely additive to SPEC-HAL-335:
// IntervalDescriptor, IntervalSource, and every existing invariant are unchanged
// (FORBID-340-1).
//
// INV-340-C1: children_of returns exactly the DIRECT children of `parent` —
//             never transitive descendants. Bounded realization (SPEC-HAL-341
//             INV-341-V2) rests entirely on this.
// INV-340-C2: Under ContainmentPolicy::Enforced, every interval whose parent is
//             P satisfies range ⊆ P.range (half-open, SPEC-HAL-334 INV-334-3).
// INV-340-C4: Source purity — TimeRange and IntervalId are the only query
//             vocabulary that crosses this trait (extends INV-335-D1).
// INV-340-C6: Containment is keyed on IntervalId (INV-335-ID1); a reference to
//             an absent id resolves as no parent / no child, never positionally.
// =============================================================================

use crate::model::time::TimeRange;

use super::descriptors::{
    IntervalDescriptor, IntervalId, IntervalSource, TemporalDescriptors, TemporalInvalidationEvent,
};

/// Containment discipline declared by a [`ContainmentSource`]. SPEC-HAL-340 §3.
///
/// Per the mandatory-policy discipline (SPEC-HAL-337 §2): a structural subsystem
/// declares its policy before implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentPolicy {
    /// `child.range ⊆ parent.range` is a substrate invariant, validated
    /// (INV-340-C2/C3). The drill-in projection (SPEC-HAL-341) requires this.
    Enforced,
    /// Topology asserts parent/child; ranges are unconstrained. Specified for a
    /// future schedule-shaped consumer; not realized by v1.
    Emergent,
}

/// Parent/child structure over the temporal substrate. SPEC-HAL-340 §4.
///
/// Independent of [`super::IntervalSource`]: a source may implement both, but a
/// containment-bearing source is not forced to implement the flat range+rows
/// `collect`, and a flat source is not forced to implement containment
/// (FORBID-340-1). Object-safe, mirroring `IntervalSource`.
pub trait ContainmentSource {
    /// The containment discipline this source guarantees.
    fn policy(&self) -> ContainmentPolicy;

    /// Append the DIRECT children of `parent` (`None` = top-level roots)
    /// intersecting `range`. Exactly one level — never transitive descendants
    /// (INV-340-C1, FORBID-340-2). Over-returning within the level is permitted;
    /// the projection culls precisely (INV-335-D2).
    fn children_of(
        &self,
        parent: Option<IntervalId>,
        range: TimeRange,
        out: &mut TemporalDescriptors,
    );

    /// Parent of `id`, for ancestry / root-path resolution. `None` at a root.
    fn parent_of(&self, id: IntervalId) -> Option<IntervalId>;

    /// Does `id` have at least one child? Drives the enterable affordance in
    /// SPEC-HAL-341. One level only.
    fn has_children(&self, id: IntervalId) -> bool;

    /// Full descriptor for `id`, regardless of visibility — the projection needs
    /// the ViewRoot's range to set the frame and ancestors' labels for the
    /// breadcrumb, neither of which the culled snapshot can answer.
    fn interval(&self, id: IntervalId) -> Option<IntervalDescriptor>;

    /// Invalidation events since the last drain. Frame-driven; data changes
    /// never force a frame (INV-335-D3, SPEC-HAL-002).
    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent>;
}

/// A free-mode [`IntervalSource`] view of a [`ContainmentSource`], rooted at a
/// chosen node. SPEC-HAL-341 §4 (free-mode data path).
///
/// The flat interval set it exposes is exactly the direct children of the
/// current root (`None` = the synthetic top-level root, whose children are the
/// top-level spans). A SpanTreeView feeds this into the unchanged SPEC-HAL-335
/// pipeline — `IntervalProjection` culls per INV-335-D2, `LaneLayoutEngine`
/// resolves same-level overlap per §8 — and re-roots it on enter/pop, so the
/// projection reuses the substrate verbatim (SPEC-HAL-341 §10) and realizes
/// exactly one containment level (INV-341-V2, sound under INV-340-C1).
///
/// The source is held as `Box<dyn ContainmentSource>` so this adapter is a
/// single concrete `IntervalSource` type regardless of the source's concrete
/// type — a generic substrate instantiates against it without carrying the
/// source's type parameter.
pub struct RootedContainmentSource {
    source: Box<dyn ContainmentSource>,
    root: Option<IntervalId>,
}

impl RootedContainmentSource {
    /// Wrap a containment source, rooted at the synthetic top-level root.
    pub fn new(source: Box<dyn ContainmentSource>) -> Self {
        Self { source, root: None }
    }

    /// The current root whose direct children are projected. `None` = top level.
    pub fn root(&self) -> Option<IntervalId> {
        self.root
    }

    /// Re-root: subsequent `collect` yields this node's direct children. The
    /// caller re-fits the time frame to the new root's range separately
    /// (SPEC-HAL-341 INV-341-V1).
    pub fn set_root(&mut self, root: Option<IntervalId>) {
        self.root = root;
    }

    /// Borrow the underlying containment source for ancestry / `has_children` /
    /// descriptor lookup (root-path, enterable mark, frame fit) — queries the
    /// flat interval snapshot cannot answer.
    pub fn containment(&self) -> &dyn ContainmentSource {
        &*self.source
    }
}

impl IntervalSource for RootedContainmentSource {
    fn collect(&self, _rows: Option<&[u64]>, range: TimeRange, out: &mut TemporalDescriptors) {
        // Free mode: rows are ignored (one implicit row, SPEC-HAL-335 §7). The
        // flat interval set is the direct children of the current root only
        // (INV-340-C1); the projection culls to `range` (INV-335-D2).
        self.source.children_of(self.root, range, out);
    }

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent> {
        self.source.drain_invalidation_events()
    }

    fn interval_exists(&self, id: IntervalId) -> bool {
        // Identity liveness across the whole tree, not just the current root's
        // children — a selected span survives navigating away (INV-335-S2,
        // INV-341-V6). The culled snapshot cannot answer this; the source can.
        self.source.interval(id).is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Trivial source: answers the contract with no spans. Sufficient to
    /// exercise the adapter's root-state mechanism and delegation wiring
    /// without constructing descriptors.
    struct EmptySource;

    impl ContainmentSource for EmptySource {
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
        fn parent_of(&self, _id: IntervalId) -> Option<IntervalId> {
            None
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

    #[test]
    fn rooted_source_tracks_root() {
        let mut s = RootedContainmentSource::new(Box::new(EmptySource));
        assert_eq!(s.root(), None);
        s.set_root(Some(IntervalId(7)));
        assert_eq!(s.root(), Some(IntervalId(7)));
        s.set_root(None);
        assert_eq!(s.root(), None);
    }

    #[test]
    fn liveness_delegates_to_source() {
        let s = RootedContainmentSource::new(Box::new(EmptySource));
        assert!(!s.interval_exists(IntervalId(1)));
    }
}
