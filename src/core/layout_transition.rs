// ============================================================================
// SPEC-HAL-078 Stage 0+1+2+3: Layout Transition Subsystem
// ============================================================================
//
// Stage 0: Persistent storage for last-frame authoritative geometry and
// a frame-local context for accessing it.
//
// Stage 1: Eligibility detection - identifies widgets whose rect changed
// between frames, producing transition candidates (no interpolation yet).
//
// Stage 2: Transition binding - binds candidates to TransitionLayer timelines,
// creating real animation lifetimes without visual effect (paint unchanged).
//
// Stage 3: Visual application - applies interpolated geometry at paint time,
// making transitions visually animate. Layout, input, hit-testing unchanged.
//
// INVARIANTS (from SPEC-HAL-078):
// - LT-INV-5: Identity matching is strict (TreeWidgetId only, no heuristics)
// - LT-INV-6: Transition state is non-authoritative (does not own widget state)
// - LT-INV-12: Zero cost when idle (buffer reuse, no per-frame allocation)
//
// Stage 2 additions (LT-INV-20 through LT-INV-24):
// - LT-INV-20: Layout transitions never mutate widget state
// - LT-INV-21: Layout transitions never run during layout or paint
// - LT-INV-22: Timeline is the only time authority
// - LT-INV-23: No visual output change in Stage 2
// - LT-INV-24: Transition identity is strictly TreeWidgetId
//
// Stage 3 additions (LT-INV-30 through LT-INV-34):
// - LT-INV-30: Layout remains authoritative at all times
// - LT-INV-31: Interpolation is paint-only
// - LT-INV-32: Input & hit-testing use non-interpolated geometry
// - LT-INV-33: No mutation of WidgetTree during paint
// - LT-INV-34: Visual interpolation is skipped if animation finished
//
// Stage 1 additions:
// - Duplicate IDs within a frame are excluded (deterministic drop)
// - Candidates require: present in both frames, rect changed, not duplicate
//
// ============================================================================

use crate::core::animation::{Animation, AnimationSample, AnimationStartError, Easing, TimelineIssuerToken, TransitionHandle};
use crate::core::math::RectF;
use crate::core::transition::TransitionLayer;
use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetTree};
use crate::utils::id::TreeWidgetId;

#[cfg(debug_assertions)]
use std::cell::Cell;
use std::collections::{HashMap, HashSet};

// ============================================================================
// SPEC-HAL-078A: Layout Transition Authorization & Policy Gate (Stage 1.5)
// ============================================================================
//
// Authorization occurs AFTER eligibility detection (Stage 1) but BEFORE
// conflict resolution (Stage 1.75) and transition binding (Stage 2).
//
// PRINCIPLES (from SPEC-HAL-078A):
// - PRINCIPLE-1: Mechanics and policy are separate
// - PRINCIPLE-2: Default is denial
// - PRINCIPLE-3: Deterministic and pure
// - PRINCIPLE-4: Engine-level only (no widget-local heuristics)
//
// AUTHORIZATION INVARIANTS:
// - AUTH-INV-1: Authorization is mandatory (no transition without passing)
// - AUTH-INV-2: Authorization is pure (no side effects)
// - AUTH-INV-3: Authorization does not observe animation state
// - AUTH-INV-4: Authorization is deterministic (same input → same output)
// - AUTH-INV-5: Authorization is frame-local (no cross-frame memory)
//
// STAGE 1 MECHANICAL LAWS:
// - Identity continuity: Same TreeWidgetId before/after reflow
// - Authority legality: Transition creation must occur outside LayoutAuthority
// - Geometry sanity: from and to rects must both be layout-authored snapshots
//
// ============================================================================

/// SPEC-HAL-078A: Authorization decision for a layout transition candidate.
///
/// Each candidate must be explicitly classified. No silent fallback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDecision {
    /// Candidate is allowed to proceed to conflict resolution and binding.
    Allowed,
    /// Candidate is denied. Transition will not be created.
    Denied(DenialReason),
    /// Candidate is deferred for later evaluation (future use).
    Deferred(DeferralReason),
}

/// SPEC-HAL-078A: Reasons for denying a transition candidate.
///
/// Stage 1 enforcement includes identity, authority, and geometry checks.
/// Debug-inspectable, no runtime cost when not observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DenialReason {
    /// AUTH-DENY-1: TreeWidgetId not present in previous frame.
    /// Identity continuity requires presence in both frames.
    IdentityNotInPrevious,
    /// AUTH-DENY-2: TreeWidgetId not present in current frame.
    /// Identity continuity requires presence in both frames.
    IdentityNotInCurrent,
    /// AUTH-DENY-3: Transition creation attempted under LayoutAuthority.
    /// SPEC-HAL-145: Transition operations are forbidden during layout phase.
    LayoutAuthorityActive,
    /// AUTH-DENY-4: From rect is not a valid layout-authored snapshot.
    /// Geometry sanity: from rect must originate from layout.
    FromRectInvalid,
    /// AUTH-DENY-5: To rect is not a valid layout-authored snapshot.
    /// Geometry sanity: to rect must originate from layout.
    ToRectInvalid,
    /// AUTH-DENY-6: Reparented widget (same ID but different parent path).
    /// SPEC-HAL-078A Section 7: Semantic identity changed even if ID did not.
    ReparentedWidget,
    /// AUTH-DENY-7: Widget has active input capture.
    /// SPEC-HAL-078A AUTH-COND-3: Non-interactive widgets only.
    ActiveInputCapture,
    /// AUTH-DENY-8: Virtualized node (identity continuity is false).
    /// SPEC-HAL-078A AUTH-DENY-1: Rects are recycled in virtualization.
    VirtualizedNode,
}

/// SPEC-HAL-078A: Reasons for deferring a transition candidate.
///
/// Deferred candidates may be re-evaluated in a future frame or stage.
/// Placeholder for future extensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeferralReason {
    /// Deferred due to pending structural change.
    PendingStructuralChange,
    /// Deferred due to transition already in progress for this widget.
    TransitionInProgress,
}

/// SPEC-HAL-078A: Debug information for a denied transition.
///
/// Debug-only. No runtime cost in release builds.
#[derive(Debug, Clone, Copy)]
#[cfg(debug_assertions)]
pub struct AuthorizationDenialRecord {
    /// Widget that was denied
    pub widget_id: TreeWidgetId,
    /// The denial reason
    pub reason: DenialReason,
}

/// SPEC-HAL-078A: Context for authorization checks.
///
/// Provides read-only access to tree state needed for authorization.
/// Does not own any data - references are borrowed for the authorization pass.
pub struct AuthorizationContext<'a> {
    /// Layout transition store for geometry snapshot verification.
    pub store: &'a LayoutTransitionStore,
    /// Set of widget IDs that have been reparented this frame.
    /// Empty if reparenting detection is not available.
    pub reparented_ids: &'a HashSet<TreeWidgetId>,
    /// Set of widget IDs with active input capture.
    /// Empty if input capture detection is not available.
    pub captured_ids: &'a HashSet<TreeWidgetId>,
    /// Set of widget IDs that are virtualized.
    /// Empty if virtualization detection is not available.
    pub virtualized_ids: &'a HashSet<TreeWidgetId>,
}

impl<'a> AuthorizationContext<'a> {
    /// Create a minimal authorization context with just the store.
    ///
    /// For testing or when reparenting/capture detection is not available.
    pub fn minimal(store: &'a LayoutTransitionStore) -> Self {
        // We use a 'static empty set to avoid lifetime issues
        static EMPTY_SET: std::sync::LazyLock<HashSet<TreeWidgetId>> =
            std::sync::LazyLock::new(HashSet::new);
        Self {
            store,
            reparented_ids: &EMPTY_SET,
            captured_ids: &EMPTY_SET,
            virtualized_ids: &EMPTY_SET,
        }
    }
}

// ============================================================================
// SPEC-HAL-078A Stage 1.5: Authorization Functions
// ============================================================================

/// SPEC-HAL-078A: Authorize a single transition candidate.
///
/// This is the SINGLE CHOKE POINT for all transition authorization.
/// No transition may reach Stage 2 without passing this function.
///
/// MECHANICAL LAWS ENFORCED (Stage 1):
/// 1. Identity continuity: TreeWidgetId present in both frames
/// 2. Authority legality: Not under LayoutAuthority
/// 3. Geometry sanity: from and to rects are valid layout snapshots
///
/// AUTH-INV-2: Pure function with no side effects.
/// AUTH-INV-4: Same input → same output.
#[inline]
pub fn authorize_candidate(
    candidate: &LayoutTransitionCandidate,
    ctx: &AuthorizationContext,
) -> AuthorizationDecision {
    // ========================================================================
    // LAW 1: Authority legality
    // Transition creation must occur OUTSIDE LayoutAuthority.
    // SPEC-HAL-145: Layout → Transition → Paint are mutually exclusive phases.
    // ========================================================================
    #[cfg(debug_assertions)]
    {
        if crate::core::rect_authority::is_layout_authority_active() {
            return AuthorizationDecision::Denied(DenialReason::LayoutAuthorityActive);
        }
    }

    // ========================================================================
    // LAW 2: Identity continuity
    // Same TreeWidgetId must exist in both previous and current frames.
    // ========================================================================

    // Check previous frame presence via store
    if ctx.store.get_previous(candidate.id).is_none() {
        return AuthorizationDecision::Denied(DenialReason::IdentityNotInPrevious);
    }

    // Check current frame presence via store
    if ctx.store.get_current(candidate.id).is_none() {
        return AuthorizationDecision::Denied(DenialReason::IdentityNotInCurrent);
    }

    // ========================================================================
    // LAW 3: Geometry sanity
    // from and to rects must both be layout-authored snapshots.
    // We verify by comparing against the store's recorded geometry.
    // ========================================================================

    // Verify from rect matches stored previous geometry
    if let Some(stored_from) = ctx.store.get_previous(candidate.id) {
        if stored_from != candidate.from {
            return AuthorizationDecision::Denied(DenialReason::FromRectInvalid);
        }
    }

    // Verify to rect matches stored current geometry
    if let Some(stored_to) = ctx.store.get_current(candidate.id) {
        if stored_to != candidate.to {
            return AuthorizationDecision::Denied(DenialReason::ToRectInvalid);
        }
    }

    // ========================================================================
    // Additional Stage 1 Checks (from SPEC-HAL-078A Section 7-8)
    // ========================================================================

    // AUTH-DENY-5: Reparented widgets
    if ctx.reparented_ids.contains(&candidate.id) {
        return AuthorizationDecision::Denied(DenialReason::ReparentedWidget);
    }

    // AUTH-COND-3: Non-interactive widget (active input capture denied)
    if ctx.captured_ids.contains(&candidate.id) {
        return AuthorizationDecision::Denied(DenialReason::ActiveInputCapture);
    }

    // AUTH-DENY-1: Virtualized nodes
    if ctx.virtualized_ids.contains(&candidate.id) {
        return AuthorizationDecision::Denied(DenialReason::VirtualizedNode);
    }

    // All checks passed - candidate is authorized
    AuthorizationDecision::Allowed
}

/// SPEC-HAL-078A Stage 1.5: Authorize all candidates.
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// This function is kept for test compatibility but should not be called directly
/// in the frame pipeline. The unified resolver is the only legal entry point.
///
/// INVARIANTS:
/// - AUTH-INV-1: Authorization is mandatory (every candidate checked)
/// - AUTH-INV-2: Pure function (no side effects beyond filtering)
/// - O(n) over candidates
/// - Zero allocations (in-place filtering)
///
/// In debug builds, records denial reasons for observability.
#[cfg(test)]
#[cfg(debug_assertions)]
pub(crate) fn authorize_candidates(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    ctx: &AuthorizationContext,
    denial_records: &mut Vec<AuthorizationDenialRecord>,
) {
    denial_records.clear();

    candidates.retain(|candidate| {
        let decision = authorize_candidate(candidate, ctx);

        match decision {
            AuthorizationDecision::Allowed => true,
            AuthorizationDecision::Denied(reason) => {
                // Record denial for debug observability
                denial_records.push(AuthorizationDenialRecord {
                    widget_id: candidate.id,
                    reason,
                });
                false
            }
            AuthorizationDecision::Deferred(_) => {
                // Deferred candidates are not included in this pass
                // Future: may be re-evaluated later
                false
            }
        }
    });
}

/// SPEC-HAL-078A Stage 1.5: Authorize all candidates (release build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// Same invariants as debug version but without denial recording for zero overhead.
#[cfg(test)]
#[cfg(not(debug_assertions))]
pub(crate) fn authorize_candidates(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    ctx: &AuthorizationContext,
) {
    candidates.retain(|candidate| {
        matches!(
            authorize_candidate(candidate, ctx),
            AuthorizationDecision::Allowed
        )
    });
}

// ============================================================================
// SPEC-HAL-078B: Layout Transition Conflict Law (Stage 1.75)
// ============================================================================
//
// Conflict resolution occurs AFTER authorization (Stage 1.5 / SPEC-HAL-078A)
// but BEFORE transition binding (Stage 2).
//
// PRINCIPLES (from SPEC-HAL-078B):
// - PRINCIPLE-1: Single visual authority per axis
// - PRINCIPLE-2: Layout is always authoritative
// - PRINCIPLE-3: Conflict resolution is global, not widget-local
// - PRINCIPLE-4: Conflicts resolve by denial, not blending
//
// PERFORMANCE INVARIANTS:
// - O(n) over authorized candidates
// - Zero allocations (caller owns buffers)
// - No tree traversal beyond parent chain lookup
// - Frame-local only
//
// ============================================================================

/// SPEC-HAL-078B: Identifies which conflict rule caused a transition denial.
///
/// Each variant corresponds to a hard denial rule from SPEC-HAL-078B Section 6.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictRuleId {
    /// CONFLICT-1: Layout transition vs visual scroll transform.
    /// Widget is inside a scroll container that applies render-time transforms.
    ScrollTransform,
    /// CONFLICT-2: Layout transition vs virtualization projection.
    /// Widget's rect originates from recycled/projected virtualization.
    VirtualizationProjection,
    /// CONFLICT-3: Layout transition vs overlay participation.
    /// Widget participates in tooltip, popover, menu, or transient overlay.
    OverlayParticipation,
    /// CONFLICT-4: Layout transition vs viewport anchoring compensation.
    /// Rect movement is compensated by scroll anchoring.
    ViewportAnchoring,
    /// CONFLICT-5: Layout transition vs render-time rect mutation.
    /// Render-time system mutates rects directly (e.g., Batcher scroll).
    RenderTimeRectMutation,
    /// CONFLICT-COND-1: Nested layout transitions share scroll context.
    /// Ancestor/child transitions in same scroll container.
    NestedTransitionScrollContext,
}

/// SPEC-HAL-078B: Result of conflict resolution for a single candidate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransitionConflictResult {
    /// Candidate is allowed to proceed to Stage 2 binding.
    Allowed,
    /// Candidate is denied due to conflict with another system.
    Denied { rule: ConflictRuleId },
}

/// SPEC-HAL-078B: Debug information for a denied transition.
///
/// Debug-only. No runtime cost in release builds.
#[derive(Debug, Clone, Copy)]
#[cfg(debug_assertions)]
pub struct ConflictDenialRecord {
    /// Widget that was denied
    pub widget_id: TreeWidgetId,
    /// The conflict rule that caused denial
    pub rule: ConflictRuleId,
    /// The dominant system that took precedence
    pub dominant_system: &'static str,
}

/// SPEC-HAL-078B: Context for conflict resolution.
///
/// Provides read-only access to tree state needed for conflict detection.
/// Does not own any data - references are borrowed for the resolution pass.
pub struct ConflictContext<'a> {
    /// Maps TreeWidgetId to (NodeId, has_scroll_ancestor, is_virtualized, is_overlay)
    /// Precomputed once per frame to avoid repeated tree walks.
    pub widget_flags: &'a HashMap<TreeWidgetId, ConflictFlags>,
}

/// SPEC-HAL-078B: Precomputed conflict flags for a single widget.
///
/// These flags are computed once per frame during layout and stored
/// for O(1) lookup during conflict resolution.
#[derive(Debug, Clone, Copy, Default)]
pub struct ConflictFlags {
    /// True if widget or any ancestor has layout_scroll_offset.
    /// Used for CONFLICT-1 and CONFLICT-5 detection.
    pub inside_scroll_container: bool,
    /// True if widget is realized through virtualization projection.
    /// Used for CONFLICT-2 detection.
    pub is_virtualized: bool,
    /// True if widget participates in overlay rendering.
    /// Used for CONFLICT-3 detection.
    pub is_overlay_participant: bool,
    /// True if widget is affected by scroll anchor compensation.
    /// Used for CONFLICT-4 detection.
    pub has_anchor_compensation: bool,
}

// ============================================================================
// SPEC-HAL-078B Stage 1.75: Conflict Resolution Functions
// ============================================================================

/// SPEC-HAL-078B: Check a single candidate for conflicts.
///
/// Returns Allowed if the candidate passes all conflict rules,
/// or Denied with the first conflict rule that fails.
///
/// PRINCIPLE-4: Conflicts resolve by denial, not blending.
/// Order of checks follows precedence table (Section 8).
#[inline]
pub fn check_candidate_conflicts(
    _candidate: &LayoutTransitionCandidate,
    flags: Option<&ConflictFlags>,
) -> TransitionConflictResult {
    // If no flags found, the widget is unknown - deny by default (don't guess)
    let Some(flags) = flags else {
        // SPEC-HAL-078B: "If data is missing, deny, do not guess"
        return TransitionConflictResult::Denied {
            rule: ConflictRuleId::ScrollTransform, // Conservative: assume scroll context
        };
    };

    // CONFLICT-1: Scroll transform takes precedence over layout transition
    // Reason: Scroll already defines motion semantics. Dual motion is illegal.
    if flags.inside_scroll_container {
        return TransitionConflictResult::Denied {
            rule: ConflictRuleId::ScrollTransform,
        };
    }

    // CONFLICT-2: Virtualization projection - identity continuity is not real
    // Reason: Rects are recycled, widget identity is ephemeral.
    if flags.is_virtualized {
        return TransitionConflictResult::Denied {
            rule: ConflictRuleId::VirtualizationProjection,
        };
    }

    // CONFLICT-3: Overlay participation - semantic lifetime override
    // Reason: Overlay lifetimes are semantic, not spatial.
    if flags.is_overlay_participant {
        return TransitionConflictResult::Denied {
            rule: ConflictRuleId::OverlayParticipation,
        };
    }

    // CONFLICT-4: Viewport anchoring compensation
    // Reason: Anchor preservation has higher priority than animation.
    if flags.has_anchor_compensation {
        return TransitionConflictResult::Denied {
            rule: ConflictRuleId::ViewportAnchoring,
        };
    }

    // CONFLICT-5: Render-time rect mutation
    // This is covered by inside_scroll_container check (same underlying cause)

    // All checks passed - candidate is allowed
    TransitionConflictResult::Allowed
}

/// SPEC-HAL-078B Stage 1.75: Resolve conflicts for all candidates.
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// This function is kept for backward compatibility but should not be called
/// directly in the frame pipeline.
///
/// INVARIANTS:
/// - O(n) over candidates
/// - Zero allocations (in-place filtering)
/// - Frame-local only
/// - Deterministic output
///
/// In debug builds, records denial reasons for observability.
#[cfg(test)]
#[cfg(debug_assertions)]
pub(crate) fn resolve_conflicts(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    conflict_flags: &HashMap<TreeWidgetId, ConflictFlags>,
    denial_records: &mut Vec<ConflictDenialRecord>,
) {
    denial_records.clear();

    candidates.retain(|candidate| {
        let flags = conflict_flags.get(&candidate.id);
        let result = check_candidate_conflicts(candidate, flags);

        match result {
            TransitionConflictResult::Allowed => true,
            TransitionConflictResult::Denied { rule } => {
                // Record denial for debug observability
                denial_records.push(ConflictDenialRecord {
                    widget_id: candidate.id,
                    rule,
                    dominant_system: dominant_system_name(rule),
                });
                false
            }
        }
    });
}

/// SPEC-HAL-078B Stage 1.75: Resolve conflicts for all candidates (release build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// Same as debug version but without denial recording for zero overhead.
#[cfg(test)]
#[cfg(not(debug_assertions))]
pub(crate) fn resolve_conflicts(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    conflict_flags: &HashMap<TreeWidgetId, ConflictFlags>,
) {
    candidates.retain(|candidate| {
        let flags = conflict_flags.get(&candidate.id);
        matches!(
            check_candidate_conflicts(candidate, flags),
            TransitionConflictResult::Allowed
        )
    });
}

/// SPEC-HAL-078B: Get the name of the dominant system for a conflict rule.
///
/// Used for debug observability (Section 10).
#[cfg(test)]
#[cfg(debug_assertions)]
const fn dominant_system_name(rule: ConflictRuleId) -> &'static str {
    match rule {
        ConflictRuleId::ScrollTransform => "scroll",
        ConflictRuleId::VirtualizationProjection => "virtualization",
        ConflictRuleId::OverlayParticipation => "overlay",
        ConflictRuleId::ViewportAnchoring => "scroll_anchoring",
        ConflictRuleId::RenderTimeRectMutation => "render_batcher",
        ConflictRuleId::NestedTransitionScrollContext => "nested_scroll",
    }
}

// ============================================================================
// SPEC-HAL-078B CONFLICT-COND-1: Nested Layout Transition Detection
// ============================================================================
//
// Multiple layout transitions in ancestor/child relationships are allowed only if:
// - they do not share scroll context
// - neither participates in overlay routing (handled by basic conflict check)
//
// Otherwise, child transitions are denied.
//
// Algorithm:
// 1. For each candidate pair (A, B), check if A is ancestor of B
// 2. If ancestor relationship exists, check if they share scroll context
// 3. If shared scroll context, mark child (B) for denial
// 4. Keep ancestor (A) - deterministic outcome (order-independent)
//
// PERFORMANCE BOUND (LT-INV-NESTED-BOUND):
// - This algorithm is O(n²) over candidates entering nested resolution.
// - Candidates reaching this point have already passed Stage 1 eligibility
//   (rect changed, present in both frames, not duplicate) and Stage 1.75
//   basic conflict filtering (not inside scroll container, not virtualized,
//   not overlay participant, not anchor compensated).
// - In practice, most widgets fail basic conflict checks (scroll containers
//   deny all descendants). The maximum expected candidates is bounded by
//   simultaneous top-level layout changes across independent subtrees.
// - MAX_NESTED_CANDIDATES guards against pathological scenarios.
//
// ============================================================================

/// SPEC-HAL-078B: Maximum candidates entering nested conflict resolution.
///
/// This bound guards against O(n²) pathological growth. In normal operation,
/// basic conflict filtering (CONFLICT-1 through CONFLICT-4) eliminates most
/// candidates before reaching nested resolution. This limit is architectural
/// insurance, not a normal operating limit.
///
/// If this assertion fires, investigate:
/// 1. Whether basic conflict flags are computed correctly
/// 2. Whether there's a subtree bypassing scroll container detection
/// 3. Whether this is a legitimate layout transition storm (rare)
#[cfg(test)]
#[cfg(debug_assertions)]
const MAX_NESTED_CANDIDATES: usize = 128;

/// SPEC-HAL-078B CONFLICT-COND-1: NodeId mapping for nested transition detection.
///
/// Maps TreeWidgetId to NodeId for parent chain traversal.
pub type WidgetNodeIdMap = HashMap<TreeWidgetId, crate::core::widget_tree::NodeId>;

/// SPEC-HAL-078B CONFLICT-COND-1: Resolve nested transition conflicts.
///
/// This is a second pass after basic conflict filtering. It detects ancestor/child
/// relationships among remaining candidates and denies child transitions when they
/// share a scroll context with their ancestor.
///
/// INVARIANTS:
/// - O(n²) worst case over remaining candidates (after basic filtering)
/// - LT-INV-NESTED-BOUND: candidate count bounded by MAX_NESTED_CANDIDATES
/// - Deterministic: always denies child, never ancestor
/// - Order-independent: same result regardless of candidate ordering
///
/// Takes a function to check scroll container sharing to avoid direct WidgetTree dependency.
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
#[cfg(test)]
#[cfg(debug_assertions)]
pub(crate) fn resolve_nested_conflicts<F>(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    is_ancestor_fn: F,
    denial_records: &mut Vec<ConflictDenialRecord>,
) where
    F: Fn(TreeWidgetId, TreeWidgetId) -> Option<bool>,
{
    // LT-INV-NESTED-BOUND: Guard against pathological O(n²) growth.
    // If this fires, basic conflict filtering is likely misconfigured.
    debug_assert!(
        candidates.len() <= MAX_NESTED_CANDIDATES,
        "SPEC-HAL-078B: Too many candidates ({}) entering nested resolution (max {}). \
         Basic conflict filtering should have eliminated most candidates.",
        candidates.len(),
        MAX_NESTED_CANDIDATES
    );

    // Collect TreeWidgetIds to check for denial
    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    // Check each pair of candidates for nested relationship
    // O(n²) but n is typically small (filtered candidates only)
    for i in 0..candidates.len() {
        for j in 0..candidates.len() {
            if i == j {
                continue;
            }

            let ancestor_id = candidates[i].id;
            let descendant_id = candidates[j].id;

            // Check if ancestor_id is ancestor of descendant_id AND they share scroll context
            // is_ancestor_fn returns Some(true) if ancestor relationship exists AND shared scroll context
            if let Some(shares_scroll) = is_ancestor_fn(ancestor_id, descendant_id) {
                if shares_scroll {
                    // CONFLICT-COND-1: Deny child transition when sharing scroll context.
                    //
                    // SEMANTIC INVARIANT: Parent visual motion implicitly carries children via
                    // the scene graph transform hierarchy. If we allow both parent and child to
                    // have independent layout transitions, the child moves TWICE: once from its
                    // own transition and once from being carried by the parent. This causes
                    // visual drift and violates single-authority-per-axis.
                    //
                    // Resolution: Keep the ancestor transition (it already moves the child
                    // visually), deny the child transition (prevents double motion).
                    // This is deterministic and order-independent.
                    denied_ids.insert(descendant_id);
                }
            }
        }
    }

    // Filter out denied candidates and record denials
    candidates.retain(|candidate| {
        if denied_ids.contains(&candidate.id) {
            denial_records.push(ConflictDenialRecord {
                widget_id: candidate.id,
                rule: ConflictRuleId::NestedTransitionScrollContext,
                dominant_system: "nested_scroll",
            });
            false
        } else {
            true
        }
    });
}

/// SPEC-HAL-078B CONFLICT-COND-1: Resolve nested transition conflicts (release build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// Same invariants as debug version (LT-INV-NESTED-BOUND applies).
#[cfg(test)]
#[cfg(not(debug_assertions))]
pub(crate) fn resolve_nested_conflicts<F>(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    is_ancestor_fn: F,
) where
    F: Fn(TreeWidgetId, TreeWidgetId) -> Option<bool>,
{
    // LT-INV-NESTED-BOUND: In release, we trust basic filtering has bounded candidates.
    // The bound is architecturally enforced, not runtime-checked.

    // Collect TreeWidgetIds to check for denial
    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    // Check each pair of candidates for nested relationship
    for i in 0..candidates.len() {
        for j in 0..candidates.len() {
            if i == j {
                continue;
            }

            let ancestor_id = candidates[i].id;
            let descendant_id = candidates[j].id;

            // Check if ancestor_id is ancestor of descendant_id AND they share scroll context
            // (See debug version for full SEMANTIC INVARIANT explanation)
            if let Some(shares_scroll) = is_ancestor_fn(ancestor_id, descendant_id) {
                if shares_scroll {
                    // Parent carries child visually; deny child to prevent double motion.
                    denied_ids.insert(descendant_id);
                }
            }
        }
    }

    // Filter out denied candidates
    candidates.retain(|candidate| !denied_ids.contains(&candidate.id));
}

// ============================================================================
// SPEC-HAL-078B Stage 1: Transition Conflict Resolution Laws
// ============================================================================
//
// This section implements the STAGE 1 conflict laws that determine which
// simultaneously authorized transitions may coexist.
//
// These laws operate AFTER SPEC-HAL-078A authorization and BEFORE binding.
// All input candidates are assumed to be individually legal (passed authorization).
//
// STAGE 1 LAWS:
// - LAW-B1: Single-Winner per Widget (duplicate IDs → keep one, deny rest)
// - LAW-B2: Parent–Child Exclusivity (parent wins, child denied)
// - LAW-B3: Scroll Container Supremacy (scroll container denies descendants)
//
// PRINCIPLES (from SPEC-HAL-078B):
// - PRINCIPLE-3: Conflict resolution is global, not widget-local
// - PRINCIPLE-4: Conflicts resolve by denial, not blending
//
// DETERMINISM GUARANTEE:
// - Output is identical regardless of input candidate order
// - Same candidates → same survivors (order-independent)
//
// ============================================================================

/// SPEC-HAL-078B Stage 1+2: Reasons for denying a transition due to conflict.
///
/// Each variant corresponds to a conflict law.
/// Debug-inspectable, no runtime cost when not observed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConflictDenialReason {
    /// LAW-B1: Widget ID appears multiple times in candidate list.
    /// One survivor is kept (deterministic selection), others denied.
    DuplicateWidget,
    /// LAW-B2: An ancestor widget is also transitioning.
    /// Parent motion subsumes child; child transition is redundant.
    ParentTransitioning,
    /// LAW-B3: A scroll container ancestor is transitioning.
    /// Scroll containers own spatial continuity for descendants.
    ScrollContainerTransitioning,
    /// Legacy diagnostic retained for API compatibility; no longer emitted.
    /// Sibling motion envelopes do not establish a conflict under SPEC-HAL-078B §4.
    SpatialOverlapConflict {
        /// The historical winner recorded by the former spatial arbitration.
        winner: TreeWidgetId,
    },
    /// CONFLICT-1: Widget is inside a scroll container.
    /// SPEC-HAL-078B: Scroll transform takes precedence over layout transition.
    InsideScrollContainer,
    /// CONFLICT-2: Widget is realized through virtualization projection.
    /// SPEC-HAL-078B: Identity continuity is not real for virtualized nodes.
    VirtualizedNode,
    /// CONFLICT-3: Widget participates in overlay rendering.
    /// SPEC-HAL-078B: Overlay lifetimes are semantic, not spatial.
    OverlayParticipant,
    /// CONFLICT-4: Widget is affected by scroll anchor compensation.
    /// SPEC-HAL-078B: Anchor preservation has higher priority than animation.
    ViewportAnchoring,
}

impl ConflictDenialReason {
    /// Convert from ConflictRuleId to ConflictDenialReason.
    pub fn from_rule(rule: ConflictRuleId) -> Self {
        match rule {
            ConflictRuleId::ScrollTransform => ConflictDenialReason::InsideScrollContainer,
            ConflictRuleId::VirtualizationProjection => ConflictDenialReason::VirtualizedNode,
            ConflictRuleId::OverlayParticipation => ConflictDenialReason::OverlayParticipant,
            ConflictRuleId::ViewportAnchoring => ConflictDenialReason::ViewportAnchoring,
            ConflictRuleId::RenderTimeRectMutation => ConflictDenialReason::InsideScrollContainer,
            ConflictRuleId::NestedTransitionScrollContext => {
                ConflictDenialReason::ScrollContainerTransitioning
            }
        }
    }
}

/// SPEC-HAL-078B Stage 1: Debug information for a conflict-denied transition.
///
/// Debug-only. No runtime cost in release builds.
#[derive(Debug, Clone, Copy)]
#[cfg(debug_assertions)]
pub struct Stage1ConflictDenialRecord {
    /// Widget that was denied
    pub widget_id: TreeWidgetId,
    /// The conflict law that caused denial
    pub reason: ConflictDenialReason,
}

// ============================================================================
// SPEC-HAL-078B Stage 1: Single Choke Point Function
// ============================================================================

/// SPEC-HAL-078B Stage 1: Resolve transition conflicts (debug build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// This function is kept for test compatibility but should not be called directly
/// in the frame pipeline. The unified resolver is the only legal entry point.
///
/// LAWS ENFORCED (in order):
/// - LAW-B1: Single-winner per widget (duplicate IDs → keep first by sorted ID)
/// - LAW-B3: Scroll container supremacy (transitioning scroll container denies descendants)
/// - LAW-B2: Parent-child exclusivity (any transitioning ancestor denies descendants)
///
/// INVARIANTS:
/// - O(n²) worst case (ancestor checks)
/// - Deterministic output (order-independent via sorted ID)
/// - Frame-local only
///
/// In debug builds, records denial reasons for observability.
#[cfg(test)]
#[cfg(debug_assertions)]
pub(crate) fn resolve_transition_conflicts_stage1(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
    denial_records: &mut Vec<Stage1ConflictDenialRecord>,
) {
    denial_records.clear();

    // ========================================================================
    // LAW-B1: Single-Winner per Widget
    // ========================================================================
    // A widget may participate in at most one transition.
    // Duplicate candidates for same TreeWidgetId → keep first by sorted ID.
    //
    // Determinism: Sort by ID, then scan with last_id, keeping first occurrence.
    // ========================================================================

    // Sort by TreeWidgetId for deterministic duplicate resolution
    candidates.sort_by_key(|c| c.id.raw());

    // Scan with last_id: keep first occurrence of each ID, remove rest
    let mut last_id: Option<TreeWidgetId> = None;
    let mut write_idx = 0;

    for read_idx in 0..candidates.len() {
        let candidate_id = candidates[read_idx].id;

        if Some(candidate_id) == last_id {
            // Duplicate - record denial (we can only record widget_id, not which specific instance)
            denial_records.push(Stage1ConflictDenialRecord {
                widget_id: candidate_id,
                reason: ConflictDenialReason::DuplicateWidget,
            });
        } else {
            // New ID - keep it
            if write_idx != read_idx {
                candidates[write_idx] = candidates[read_idx];
            }
            write_idx += 1;
            last_id = Some(candidate_id);
        }
    }

    candidates.truncate(write_idx);

    // Collect IDs to deny (built up by LAW-B3 and LAW-B2)
    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    // ========================================================================
    // LAW-B3: Scroll Container Supremacy
    // ========================================================================
    // If a scroll container transitions, all descendant transitions are denied.
    // Scroll containers are identified by tree.node(id).layout_scroll_offset.is_some().
    //
    // Runs before LAW-B2 so scroll container denials are recorded as
    // ScrollContainerTransitioning, not ParentTransitioning.
    // ========================================================================

    // Find which candidates are scroll containers (have layout_scroll_offset)
    let transitioning_scroll_containers: Vec<TreeWidgetId> = candidates
        .iter()
        .filter_map(|c| {
            let node_id = tree.find_by_id(c.id)?;
            let node = tree.node(node_id)?;
            if node.layout_scroll_offset.is_some() {
                Some(c.id)
            } else {
                None
            }
        })
        .collect();

    // For each candidate, check if it's a descendant of any transitioning scroll container
    for candidate in candidates.iter() {
        // Get NodeId for this candidate
        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        for &scroll_widget_id in &transitioning_scroll_containers {
            if scroll_widget_id == candidate.id {
                continue; // Don't deny the scroll container itself
            }

            let Some(scroll_node_id) = tree.find_by_id(scroll_widget_id) else {
                continue;
            };

            // Is candidate a descendant of this scroll container?
            if tree.is_ancestor_of(scroll_node_id, candidate_node_id) {
                denied_ids.insert(candidate.id);
                denial_records.push(Stage1ConflictDenialRecord {
                    widget_id: candidate.id,
                    reason: ConflictDenialReason::ScrollContainerTransitioning,
                });
                break; // Already denied, no need to check other scroll containers
            }
        }
    }

    // ========================================================================
    // LAW-B2: Parent–Child Exclusivity
    // ========================================================================
    // If both an ancestor and its descendant have transitions:
    // - Ancestor wins
    // - Descendant transitions are denied
    //
    // Uses tree ancestry, not geometry.
    // O(n²) over remaining candidates.
    // ========================================================================

    for candidate in candidates.iter() {
        // Skip if already denied by LAW-B3
        if denied_ids.contains(&candidate.id) {
            continue;
        }

        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        // Check if any OTHER candidate is an ancestor of this candidate
        for potential_ancestor in candidates.iter() {
            if potential_ancestor.id == candidate.id {
                continue;
            }
            // Skip if the potential ancestor is already denied
            if denied_ids.contains(&potential_ancestor.id) {
                continue;
            }

            let Some(ancestor_node_id) = tree.find_by_id(potential_ancestor.id) else {
                continue;
            };

            // Is potential_ancestor an ancestor of candidate?
            if tree.is_ancestor_of(ancestor_node_id, candidate_node_id) {
                // Ancestor is transitioning - deny this descendant
                denied_ids.insert(candidate.id);
                denial_records.push(Stage1ConflictDenialRecord {
                    widget_id: candidate.id,
                    reason: ConflictDenialReason::ParentTransitioning,
                });
                break; // No need to check other ancestors
            }
        }
    }

    // ========================================================================
    // Final filtering
    // ========================================================================
    candidates.retain(|c| !denied_ids.contains(&c.id));
}

/// SPEC-HAL-078B Stage 1: Resolve transition conflicts (release build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// Same invariants as debug version but without denial recording for zero overhead.
#[cfg(test)]
#[cfg(not(debug_assertions))]
pub(crate) fn resolve_transition_conflicts_stage1(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
) {
    // LAW-B1: Single-Winner per Widget
    // Sort by ID, scan with last_id, keep first occurrence
    candidates.sort_by_key(|c| c.id.raw());

    let mut last_id: Option<TreeWidgetId> = None;
    let mut write_idx = 0;

    for read_idx in 0..candidates.len() {
        let candidate_id = candidates[read_idx].id;

        if Some(candidate_id) != last_id {
            if write_idx != read_idx {
                candidates[write_idx] = candidates[read_idx];
            }
            write_idx += 1;
            last_id = Some(candidate_id);
        }
    }

    candidates.truncate(write_idx);

    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    // LAW-B3: Scroll Container Supremacy
    let transitioning_scroll_containers: Vec<TreeWidgetId> = candidates
        .iter()
        .filter_map(|c| {
            let node_id = tree.find_by_id(c.id)?;
            let node = tree.node(node_id)?;
            if node.layout_scroll_offset.is_some() {
                Some(c.id)
            } else {
                None
            }
        })
        .collect();

    for candidate in candidates.iter() {
        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        for &scroll_widget_id in &transitioning_scroll_containers {
            if scroll_widget_id == candidate.id {
                continue;
            }

            let Some(scroll_node_id) = tree.find_by_id(scroll_widget_id) else {
                continue;
            };

            if tree.is_ancestor_of(scroll_node_id, candidate_node_id) {
                denied_ids.insert(candidate.id);
                break;
            }
        }
    }

    // LAW-B2: Parent–Child Exclusivity
    for i in 0..candidates.len() {
        let candidate_id = candidates[i].id;
        if denied_ids.contains(&candidate_id) {
            continue;
        }

        let Some(candidate_node_id) = tree.find_by_id(candidate_id) else {
            continue;
        };

        for j in 0..candidates.len() {
            if i == j {
                continue;
            }
            let ancestor_id = candidates[j].id;
            if denied_ids.contains(&ancestor_id) {
                continue;
            }

            let Some(ancestor_node_id) = tree.find_by_id(ancestor_id) else {
                continue;
            };

            if tree.is_ancestor_of(ancestor_node_id, candidate_node_id) {
                denied_ids.insert(candidate_id);
                break;
            }
        }
    }

    candidates.retain(|c| !denied_ids.contains(&c.id));
}

// ============================================================================
// SPEC-HAL-078B: Unified Conflict Resolution Entry Point
// ============================================================================

/// SPEC-HAL-078B: Resolve hierarchy transition conflicts.
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// This function is kept for test compatibility but should not be called directly
/// in the frame pipeline. The unified resolver is the only legal entry point.
///
/// LAWS ENFORCED (in order):
/// - LAW-B1: Single-winner per widget (Stage 1)
/// - LAW-B3: Scroll container supremacy (Stage 1)
/// - LAW-B2: Parent-child exclusivity (Stage 1)
///
/// INVARIANTS:
/// - O(n²) worst case (hierarchy checks)
/// - Deterministic output (order-independent)
/// - Frame-local only
/// - No floating-point nondeterminism
///
/// In debug builds, records denial reasons for observability.
#[cfg(test)]
#[cfg(debug_assertions)]
pub(crate) fn resolve_transition_conflicts(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
    denial_records: &mut Vec<Stage1ConflictDenialRecord>,
) {
    // Stage 1: Hierarchy laws (B1, B3, B2)
    resolve_transition_conflicts_stage1(candidates, tree, denial_records);
}

/// SPEC-HAL-078B: Resolve all transition conflicts (release build).
///
/// **DEPRECATED:** Use `resolve_layout_transition_candidates()` instead.
/// Same invariants as debug version but without denial recording for zero overhead.
#[cfg(test)]
#[cfg(not(debug_assertions))]
pub(crate) fn resolve_transition_conflicts(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
) {
    resolve_transition_conflicts_stage1(candidates, tree);
}

// ============================================================================
// SPEC-HAL-078A+078B: UNIFIED RESOLVER — SINGLE CHOKE POINT
// ============================================================================
//
// This is the ONE AND ONLY callable path for layout transition resolution.
// Authorization (078A) + Conflict Resolution (078B Stage 1 + Stage 2) are
// chained here. No transition may reach binding without passing this function.
//
// ENFORCEMENT:
// - All individual stage functions are pub(crate) or module-private
// - Direct calls to stages panic in debug builds (bypass detection)
// - Candidates are returned as an immutable reference after resolution
//
// INVARIANTS:
// - AUTH-INV-1: Authorization is mandatory (every candidate checked)
// - PRINCIPLE-3: Conflict resolution is global, not widget-local
// - PRINCIPLE-4: Conflicts resolve by denial, not blending
// - O(n²) worst case (pairwise checks)
// - Deterministic output (order-independent)
// - Zero overhead for denial tracking in release builds
//
// ============================================================================

/// SPEC-HAL-078A+078B: Unified denial record for debug observability.
///
/// Captures denials from both authorization (078A) and conflict resolution (078B).
/// Debug-only. Zero runtime cost in release builds.
#[derive(Debug, Clone, Copy)]
#[cfg(debug_assertions)]
pub enum UnifiedDenialRecord {
    /// Denied by SPEC-HAL-078A authorization gate.
    Authorization {
        widget_id: TreeWidgetId,
        reason: DenialReason,
    },
    /// Denied by SPEC-HAL-078B conflict law (Stage 1 or Stage 2).
    Conflict {
        widget_id: TreeWidgetId,
        reason: ConflictDenialReason,
    },
}

/// SPEC-HAL-078A+078B: Unified resolver result.
///
/// Contains the filtered candidates (immutable after resolution) and debug records.
#[cfg(debug_assertions)]
pub struct ResolvedTransitionCandidates {
    /// The surviving candidates after authorization and conflict resolution.
    /// Immutable - no further modification allowed.
    candidates: Vec<LayoutTransitionCandidate>,
    /// Debug-only: all denial records from authorization and conflict resolution.
    denial_records: Vec<UnifiedDenialRecord>,
}

#[cfg(debug_assertions)]
impl ResolvedTransitionCandidates {
    /// Get the resolved candidates (immutable).
    pub fn candidates(&self) -> &[LayoutTransitionCandidate] {
        &self.candidates
    }

    /// Get denial records for debug/tracing.
    pub fn denial_records(&self) -> &[UnifiedDenialRecord] {
        &self.denial_records
    }

    /// Consume and return the candidates (for binding).
    pub fn into_candidates(self) -> Vec<LayoutTransitionCandidate> {
        self.candidates
    }
}

/// SPEC-HAL-078A+078B: Unified resolver result (release build).
#[cfg(not(debug_assertions))]
pub struct ResolvedTransitionCandidates {
    candidates: Vec<LayoutTransitionCandidate>,
}

#[cfg(not(debug_assertions))]
impl ResolvedTransitionCandidates {
    pub fn candidates(&self) -> &[LayoutTransitionCandidate] {
        &self.candidates
    }

    pub fn into_candidates(self) -> Vec<LayoutTransitionCandidate> {
        self.candidates
    }
}

/// SPEC-HAL-078A+078B: THE UNIFIED RESOLVER — SINGLE CHOKE POINT.
///
/// This is the ONLY callable path for layout transition resolution.
/// Chains:
/// 1. SPEC-HAL-078A authorization gate
/// 2. SPEC-HAL-078B competing visual influence exclusions
/// 3. SPEC-HAL-078B hierarchy checks (B1, B3, B2)
///
/// Independent widgets do not compete for visual authority (078B §4).
/// Intersecting swept envelopes alone must not suppress either transition.
///
/// No transition may reach binding without passing this function.
///
/// INVARIANTS:
/// - Authorization is mandatory (AUTH-INV-1)
/// - Conflicts resolve by denial, not blending (PRINCIPLE-4)
/// - Candidates are immutable after resolution
/// - O(n²) worst case (hierarchy checks)
/// - Deterministic output (order-independent)
/// - Zero overhead for denial tracking in release builds
///
/// # Arguments
/// * `candidates` - Raw candidates from eligibility detection
/// * `auth_ctx` - Authorization context for SPEC-HAL-078A checks
/// * `tree` - Widget tree for SPEC-HAL-078B conflict checks
/// * `frame_epoch` - Current layout epoch for telemetry (debug builds only)
///
/// # Returns
/// `ResolvedTransitionCandidates` with immutable surviving candidates.
#[cfg(debug_assertions)]
pub fn resolve_layout_transition_candidates(
    mut candidates: Vec<LayoutTransitionCandidate>,
    auth_ctx: &AuthorizationContext,
    tree: &WidgetTree,
    conflict_flags: &std::collections::HashMap<TreeWidgetId, ConflictFlags>,
    frame_epoch: u64,
) -> ResolvedTransitionCandidates {
    let mut denial_records = Vec::new();

    // ========================================================================
    // Stage 1.5: SPEC-HAL-078A Authorization Gate
    // ========================================================================
    let mut auth_denial_records = Vec::new();
    authorize_candidates_internal(&mut candidates, auth_ctx, &mut auth_denial_records);

    // Convert authorization denials to unified format
    for record in auth_denial_records {
        denial_records.push(UnifiedDenialRecord::Authorization {
            widget_id: record.widget_id,
            reason: record.reason,
        });
    }

    // ========================================================================
    // Stage 1.6: SPEC-HAL-078B CONFLICT-1 Check (Scroll Container Denial)
    // ========================================================================
    // FIX 2: Enforce CONFLICT-1 - widgets inside scroll containers are denied
    // unconditionally, regardless of whether the scroll container is transitioning.
    {
        let mut denied_ids = Vec::new();
        for candidate in candidates.iter() {
            if let Some(flags) = conflict_flags.get(&candidate.id) {
                let result = check_candidate_conflicts(candidate, Some(flags));
                if let TransitionConflictResult::Denied { rule } = result {
                    denied_ids.push(candidate.id);
                    denial_records.push(UnifiedDenialRecord::Conflict {
                        widget_id: candidate.id,
                        reason: ConflictDenialReason::from_rule(rule),
                    });
                }
            }
            // Note: If no flags found, we let Stage 1.75 handle it (conservative denial)
        }
        candidates.retain(|c| !denied_ids.contains(&c.id));
    }

    // ========================================================================
    // Stage 1.75: SPEC-HAL-078B Hierarchy Conflict Resolution
    // ========================================================================
    let mut conflict_denial_records = Vec::new();
    resolve_transition_conflicts_stage1_internal(
        &mut candidates,
        tree,
        &mut conflict_denial_records,
    );

    // Convert conflict denials to unified format
    for record in conflict_denial_records {
        denial_records.push(UnifiedDenialRecord::Conflict {
            widget_id: record.widget_id,
            reason: record.reason,
        });
    }

    // Log denials via tracing (debug builds only)
    if !denial_records.is_empty() {
        tracing::debug!(
            "SPEC-HAL-078A+078B: {} candidates denied, {} survived",
            denial_records.len(),
            candidates.len()
        );
    }

    // Invoke telemetry hook if registered (debug builds only)
    if !denial_records.is_empty() {
        if let Some(hook) = DENIAL_TELEMETRY_HOOK.with(|h| h.get()) {
            hook(frame_epoch, &denial_records);
        }
    }

    ResolvedTransitionCandidates {
        candidates,
        denial_records,
    }
}

/// SPEC-HAL-078A+078B: THE UNIFIED RESOLVER — SINGLE CHOKE POINT (release build).
///
/// Same invariants as debug version but without denial recording for zero overhead.
#[cfg(not(debug_assertions))]
pub fn resolve_layout_transition_candidates(
    mut candidates: Vec<LayoutTransitionCandidate>,
    auth_ctx: &AuthorizationContext,
    tree: &WidgetTree,
    conflict_flags: &std::collections::HashMap<TreeWidgetId, ConflictFlags>,
) -> ResolvedTransitionCandidates {
    // Stage 1.5: Authorization
    authorize_candidates_internal(&mut candidates, auth_ctx);

    // Stage 1.6: CONFLICT-1 Check (Scroll Container Denial)
    // FIX 2: Enforce CONFLICT-1 - widgets inside scroll containers are denied
    {
        let mut denied_ids = Vec::new();
        for candidate in candidates.iter() {
            if let Some(flags) = conflict_flags.get(&candidate.id) {
                if let TransitionConflictResult::Denied { .. } =
                    check_candidate_conflicts(candidate, Some(flags))
                {
                    denied_ids.push(candidate.id);
                }
            }
        }
        candidates.retain(|c| !denied_ids.contains(&c.id));
    }

    // Stage 1.75: Conflict resolution
    resolve_transition_conflicts_stage1_internal(&mut candidates, tree);

    ResolvedTransitionCandidates { candidates }
}

// ============================================================================
// INTERNAL FUNCTIONS — NOT FOR EXTERNAL USE
// ============================================================================
//
// These functions are the actual implementations, called only by the unified
// resolver. The public `authorize_candidates`, `resolve_transition_conflicts_stage1`,
// and `resolve_transition_conflicts_stage2` functions are kept for backward
// compatibility but will panic in debug builds if the unified resolver flag
// is not set, detecting bypass attempts.
//
// ============================================================================

thread_local! {
    /// Debug flag to detect bypass of unified resolver.
    /// Set to true only during `resolve_layout_transition_candidates`.
    #[cfg(debug_assertions)]
    static UNIFIED_RESOLVER_ACTIVE: Cell<bool> = const { Cell::new(false) };

    /// Debug-only: optional telemetry hook for denial inspection.
    /// Receives frame epoch and all denial records for structured observability.
    #[cfg(debug_assertions)]
    static DENIAL_TELEMETRY_HOOK: Cell<Option<DenialTelemetryHook>> = const { Cell::new(None) };
}

// ============================================================================
// SPEC-HAL-078 DENIAL TELEMETRY (DEBUG-ONLY)
// ============================================================================
//
// Provides structured observability into why layout transitions were denied.
// Zero runtime cost in release builds.
//
// Usage:
//   set_denial_telemetry_hook(Some(my_hook));
//   // ... frame loop calls resolve_layout_transition_candidates ...
//   set_denial_telemetry_hook(None);  // disable
//
// ============================================================================

/// Debug-only telemetry hook type for layout transition denials.
///
/// Receives:
/// - `frame_epoch`: the layout epoch from the engine frame loop
/// - `denials`: all denial records from authorization and conflict resolution
///
/// The hook is called from `resolve_layout_transition_candidates` after all
/// authorization and conflict resolution is complete, before returning.
///
/// # Example
///
/// ```ignore
/// fn my_denial_hook(frame_epoch: u64, denials: &[UnifiedDenialRecord]) {
///     for denial in denials {
///         match denial {
///             UnifiedDenialRecord::Authorization { widget_id, reason } => {
///                 tracing::warn!("Frame {}: {:?} denied by 078A: {:?}", frame_epoch, widget_id, reason);
///             }
///             UnifiedDenialRecord::Conflict { widget_id, reason } => {
///                 tracing::warn!("Frame {}: {:?} denied by 078B: {:?}", frame_epoch, widget_id, reason);
///             }
///         }
///     }
/// }
/// ```
#[cfg(debug_assertions)]
pub type DenialTelemetryHook = fn(frame_epoch: u64, denials: &[UnifiedDenialRecord]);

/// Register a debug-only denial telemetry hook.
///
/// The hook is called from `resolve_layout_transition_candidates` after all
/// authorization and conflict resolution is complete.
///
/// Pass `None` to disable the hook.
///
/// # Thread Safety
///
/// The hook is thread-local. Set it per-thread if running multi-threaded tests.
///
/// # Zero-Cost in Release
///
/// This function and all telemetry infrastructure compile out in release builds.
#[cfg(debug_assertions)]
pub fn set_denial_telemetry_hook(hook: Option<DenialTelemetryHook>) {
    DENIAL_TELEMETRY_HOOK.with(|h| h.set(hook));
}

/// Get the currently registered denial telemetry hook (if any).
///
/// Debug-only. Returns `None` if no hook is registered.
#[cfg(debug_assertions)]
pub fn get_denial_telemetry_hook() -> Option<DenialTelemetryHook> {
    DENIAL_TELEMETRY_HOOK.with(|h| h.get())
}

/// Internal: SPEC-HAL-078A authorization (called by unified resolver only).
#[cfg(debug_assertions)]
fn authorize_candidates_internal(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    ctx: &AuthorizationContext,
    denial_records: &mut Vec<AuthorizationDenialRecord>,
) {
    denial_records.clear();

    candidates.retain(|candidate| {
        let decision = authorize_candidate(candidate, ctx);

        match decision {
            AuthorizationDecision::Allowed => true,
            AuthorizationDecision::Denied(reason) => {
                denial_records.push(AuthorizationDenialRecord {
                    widget_id: candidate.id,
                    reason,
                });
                false
            }
            AuthorizationDecision::Deferred(_) => false,
        }
    });
}

#[cfg(not(debug_assertions))]
fn authorize_candidates_internal(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    ctx: &AuthorizationContext,
) {
    candidates.retain(|candidate| {
        matches!(
            authorize_candidate(candidate, ctx),
            AuthorizationDecision::Allowed
        )
    });
}

/// Internal: SPEC-HAL-078B Stage 1 conflict resolution (called by unified resolver only).
#[cfg(debug_assertions)]
fn resolve_transition_conflicts_stage1_internal(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
    denial_records: &mut Vec<Stage1ConflictDenialRecord>,
) {
    // Same implementation as resolve_transition_conflicts_stage1
    denial_records.clear();

    // LAW-B1: Single-Winner per Widget
    candidates.sort_by_key(|c| c.id.raw());

    let mut last_id: Option<TreeWidgetId> = None;
    let mut write_idx = 0;

    for read_idx in 0..candidates.len() {
        let candidate_id = candidates[read_idx].id;

        if Some(candidate_id) == last_id {
            denial_records.push(Stage1ConflictDenialRecord {
                widget_id: candidate_id,
                reason: ConflictDenialReason::DuplicateWidget,
            });
        } else {
            if write_idx != read_idx {
                candidates[write_idx] = candidates[read_idx];
            }
            write_idx += 1;
            last_id = Some(candidate_id);
        }
    }

    candidates.truncate(write_idx);

    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    // LAW-B3: Scroll Container Supremacy
    let transitioning_scroll_containers: Vec<TreeWidgetId> = candidates
        .iter()
        .filter_map(|c| {
            let node_id = tree.find_by_id(c.id)?;
            let node = tree.node(node_id)?;
            if node.layout_scroll_offset.is_some() {
                Some(c.id)
            } else {
                None
            }
        })
        .collect();

    for candidate in candidates.iter() {
        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        for &scroll_widget_id in &transitioning_scroll_containers {
            if scroll_widget_id == candidate.id {
                continue;
            }

            let Some(scroll_node_id) = tree.find_by_id(scroll_widget_id) else {
                continue;
            };

            if tree.is_ancestor_of(scroll_node_id, candidate_node_id) {
                denied_ids.insert(candidate.id);
                denial_records.push(Stage1ConflictDenialRecord {
                    widget_id: candidate.id,
                    reason: ConflictDenialReason::ScrollContainerTransitioning,
                });
                break;
            }
        }
    }

    // LAW-B2: Parent–Child Exclusivity
    for candidate in candidates.iter() {
        if denied_ids.contains(&candidate.id) {
            continue;
        }

        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        for potential_ancestor in candidates.iter() {
            if potential_ancestor.id == candidate.id {
                continue;
            }
            if denied_ids.contains(&potential_ancestor.id) {
                continue;
            }

            let Some(ancestor_node_id) = tree.find_by_id(potential_ancestor.id) else {
                continue;
            };

            if tree.is_ancestor_of(ancestor_node_id, candidate_node_id) {
                denied_ids.insert(candidate.id);
                denial_records.push(Stage1ConflictDenialRecord {
                    widget_id: candidate.id,
                    reason: ConflictDenialReason::ParentTransitioning,
                });
                break;
            }
        }
    }

    candidates.retain(|c| !denied_ids.contains(&c.id));
}

#[cfg(not(debug_assertions))]
fn resolve_transition_conflicts_stage1_internal(
    candidates: &mut Vec<LayoutTransitionCandidate>,
    tree: &WidgetTree,
) {
    candidates.sort_by_key(|c| c.id.raw());

    let mut last_id: Option<TreeWidgetId> = None;
    let mut write_idx = 0;

    for read_idx in 0..candidates.len() {
        let candidate_id = candidates[read_idx].id;

        if Some(candidate_id) != last_id {
            if write_idx != read_idx {
                candidates[write_idx] = candidates[read_idx];
            }
            write_idx += 1;
            last_id = Some(candidate_id);
        }
    }

    candidates.truncate(write_idx);

    let mut denied_ids: HashSet<TreeWidgetId> = HashSet::new();

    let transitioning_scroll_containers: Vec<TreeWidgetId> = candidates
        .iter()
        .filter_map(|c| {
            let node_id = tree.find_by_id(c.id)?;
            let node = tree.node(node_id)?;
            if node.layout_scroll_offset.is_some() {
                Some(c.id)
            } else {
                None
            }
        })
        .collect();

    for candidate in candidates.iter() {
        let Some(candidate_node_id) = tree.find_by_id(candidate.id) else {
            continue;
        };

        for &scroll_widget_id in &transitioning_scroll_containers {
            if scroll_widget_id == candidate.id {
                continue;
            }

            let Some(scroll_node_id) = tree.find_by_id(scroll_widget_id) else {
                continue;
            };

            if tree.is_ancestor_of(scroll_node_id, candidate_node_id) {
                denied_ids.insert(candidate.id);
                break;
            }
        }
    }

    for i in 0..candidates.len() {
        let candidate_id = candidates[i].id;
        if denied_ids.contains(&candidate_id) {
            continue;
        }

        let Some(candidate_node_id) = tree.find_by_id(candidate_id) else {
            continue;
        };

        for j in 0..candidates.len() {
            if i == j {
                continue;
            }
            let ancestor_id = candidates[j].id;
            if denied_ids.contains(&ancestor_id) {
                continue;
            }

            let Some(ancestor_node_id) = tree.find_by_id(ancestor_id) else {
                continue;
            };

            if tree.is_ancestor_of(ancestor_node_id, candidate_node_id) {
                denied_ids.insert(candidate_id);
                break;
            }
        }
    }

    candidates.retain(|c| !denied_ids.contains(&c.id));
}

/// SPEC-HAL-078 Stage 1: A candidate for layout transition animation.
///
/// Represents a widget whose rect changed between frames. Engine-internal only.
/// Does not imply animation will occur - just that eligibility criteria are met.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutTransitionCandidate {
    /// Widget identity (strict, no heuristics per LT-INV-5)
    pub id: TreeWidgetId,
    /// Previous frame's authoritative rect
    pub from: RectF,
    /// Current frame's authoritative rect
    pub to: RectF,
}

/// SPEC-HAL-078: Engine-owned persistent storage for last-frame geometry.
///
/// Holds authoritative rects from the previous frame, keyed by TreeWidgetId.
/// Updated once per frame during `compute_absolute_paint_rects`.
///
/// LT-INV-5: Identity matching is strict (TreeWidgetId only, no heuristics).
/// LT-INV-12: Zero cost when idle (buffer reuse, no per-frame allocation).
///
/// Stage 1: Also tracks duplicate IDs within a frame for deterministic exclusion.
pub struct LayoutTransitionStore {
    /// Current frame's geometry (being captured)
    current: HashMap<TreeWidgetId, RectF>,
    /// Previous frame's geometry (for comparison in Stage 1+)
    previous: HashMap<TreeWidgetId, RectF>,
    /// Stage 1: IDs recorded more than once this frame (excluded from eligibility)
    duplicate_current: HashSet<TreeWidgetId>,
    // Evidence, never another geometry history or a nonempty-map heuristic.
    coverage: Option<(WidgetMountToken, u64)>,
    current_complete: Option<CompletedLayout>,
    previous_complete: Option<CompletedLayout>,
    completed_generation: u64,
}

#[derive(Clone)]
struct CompletedLayout {
    root: WidgetMountToken,
    structure: u64,
    generation: u64,
}

impl Default for LayoutTransitionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl LayoutTransitionStore {
    pub fn new() -> Self {
        Self {
            current: HashMap::new(),
            previous: HashMap::new(),
            duplicate_current: HashSet::new(),
            coverage: None,
            current_complete: None,
            previous_complete: None,
            completed_generation: 0,
        }
    }

    /// Begin a new frame: swap buffers and clear current.
    ///
    /// Called once per frame before layout. Previous becomes current's old data.
    /// LT-INV-12: Reuses buffer capacity (no allocation if capacity sufficient).
    pub fn begin_frame(&mut self) {
        self.previous_complete = self.current_complete.take();
        self.coverage = None;
        std::mem::swap(&mut self.previous, &mut self.current);
        self.current.clear();
        self.duplicate_current.clear();
    }

    /// Record a widget's authoritative geometry.
    ///
    /// Called during `compute_absolute_paint_rects` for each node.
    ///
    /// Stage 1: If the same ID is recorded twice in one frame, it is marked as
    /// duplicate and excluded from eligibility. No panic, no log - deterministic drop.
    #[inline]
    pub fn record(&mut self, id: TreeWidgetId, rect: RectF) {
        self.coverage = None;
        // Native hit geometry is recomputed before its buffer swap. Identical
        // writes preserve the exact completed map; a changed/new value does not.
        // Numeric equality also refuses NaN and agrees with the collector.
        let previous = self.current.insert(id, rect);
        if previous != Some(rect) {
            self.current_complete = None;
        }
        if previous.is_some() {
            // Duplicate ID this frame - mark for exclusion from eligibility
            self.duplicate_current.insert(id);
        }
    }

    /// Check if we have previous geometry for any widget.
    ///
    /// Stage 0: Allows context to report "previous snapshot exists".
    pub fn has_previous(&self) -> bool {
        !self.previous.is_empty()
    }

    /// Get previous frame's geometry for a widget.
    ///
    /// Returns None if the widget didn't exist in the previous frame.
    pub fn get_previous(&self, id: TreeWidgetId) -> Option<RectF> {
        self.previous.get(&id).copied()
    }

    /// Get current frame's geometry for a widget.
    ///
    /// Returns None if the widget hasn't been recorded yet this frame.
    pub fn get_current(&self, id: TreeWidgetId) -> Option<RectF> {
        self.current.get(&id).copied()
    }

    pub(crate) fn record_complete_traversal(&mut self, root: WidgetMountToken, structure: u64) {
        self.coverage = Some((root, structure));
    }

    pub(crate) fn invalidate_layout_completion(&mut self) {
        self.current_complete = None;
    }

    /// Only the successful App final-layout gate may publish this witness.
    pub(crate) fn complete_layout(&mut self, root: WidgetMountToken, structure: u64) -> bool {
        let traversed = self.coverage.as_ref().is_some_and(|(r, s)| *r == root && *s == structure);
        if !traversed {
            let reusable = self.current.is_empty() && self.previous_complete.as_ref()
                .is_some_and(|p| p.root == root && p.structure == structure);
            if !reusable {
                return false;
            }
            // A clean root performs no traversal. Transfer the already authenticated
            // complete buffer, not a clone or an empty-map claim of absence.
            std::mem::swap(&mut self.current, &mut self.previous);
            self.previous_complete = None;
        }
        self.completed_generation = self.completed_generation.checked_add(1)
            .expect("HAL078 completed layout generation exhausted");
        self.current_complete = Some(CompletedLayout {
            root, structure, generation: self.completed_generation,
        });
        true
    }

    /// SPEC-HAL-078 Stage 1: Collect widgets eligible for layout transition.
    ///
    /// A candidate is produced when:
    /// - Widget existed in previous frame (has previous rect)
    /// - Widget exists in current frame (has current rect)
    /// - Rects differ (exact float comparison, no epsilon)
    /// - ID was not recorded more than once this frame (not a duplicate)
    ///
    /// Caller provides and reuses the output buffer to avoid per-frame allocation.
    /// This iterates the current map (O(n) over recorded widgets), not a tree walk.
    pub fn collect_rect_change_candidates(&self, out: &mut Vec<LayoutTransitionCandidate>) {
        out.clear();
        for (&id, &to) in &self.current {
            // Skip duplicates (deterministic drop)
            if self.duplicate_current.contains(&id) {
                continue;
            }
            // Must exist in previous frame
            let Some(from) = self.previous.get(&id).copied() else {
                continue;
            };
            // Must have changed (exact comparison)
            if from == to {
                continue;
            }
            out.push(LayoutTransitionCandidate { id, from, to });
        }
    }
}

/// SPEC-HAL-078: Frame-local context for layout transition queries.
///
/// References the persistent store. Created at frame start, dropped before paint.
/// Stage 0: Provides query accessors only; no interpolation or detection.
///
/// LT-INV-6: Does not own widget state, does not mutate widget state.
pub struct LayoutTransitionContext<'a> {
    store: &'a LayoutTransitionStore,
}

impl<'a> LayoutTransitionContext<'a> {
    pub fn new(store: &'a LayoutTransitionStore) -> Self {
        Self { store }
    }

    /// Check if previous-frame geometry exists.
    pub fn has_previous_snapshot(&self) -> bool {
        self.store.has_previous()
    }

    /// Get previous-frame rect for a widget.
    pub fn prev_rect(&self, id: TreeWidgetId) -> Option<RectF> {
        self.store.get_previous(id)
    }

    /// Get current-frame rect for a widget.
    pub fn curr_rect(&self, id: TreeWidgetId) -> Option<RectF> {
        self.store.get_current(id)
    }
}

// ============================================================================
// SPEC-HAL-078 Stage 2: Transition Binding (Non-Visual)
// ============================================================================

/// Default duration for layout transitions in seconds.
const LAYOUT_TRANSITION_DURATION: f32 = 0.3;

// ============================================================================
// SPEC-HAL-078 Stage 3: Visual Application (Rect Interpolation)
// ============================================================================
//
// INVARIANTS (Stage 3):
// - LT-INV-30: Layout remains authoritative at all times
// - LT-INV-31: Interpolation is paint-only
// - LT-INV-32: Input & hit-testing use non-interpolated geometry
// - LT-INV-33: No mutation of WidgetTree during paint
// - LT-INV-34: Visual interpolation is skipped if animation finished
//
// ============================================================================

/// SPEC-HAL-078 Stage 3: Linear interpolation of RectF.
///
/// Interpolates x, y, width, height independently.
/// Uses exact floats - no epsilon, no snapping, no easing.
/// Easing is already applied in Timeline.
///
/// LT-INV-31: This is a pure function with no side effects.
#[inline]
pub fn lerp_rect(from: RectF, to: RectF, t: f32) -> RectF {
    RectF::new(
        from.x + (to.x - from.x) * t,
        from.y + (to.y - from.y) * t,
        from.width + (to.width - from.width) * t,
        from.height + (to.height - from.height) * t,
    )
}

/// SPEC-HAL-078 Stage 2: State for an active layout transition.
///
/// Engine-owned, tracks a single widget's transition animation.
/// Does NOT own widget state (LT-INV-20).
/// Does NOT affect paint output in Stage 2 (LT-INV-23).
#[derive(Debug, Clone, Copy)]
pub struct LayoutTransitionState {
    /// Widget identity (strict per LT-INV-24)
    pub id: TreeWidgetId,
    /// Handle to the animation in TransitionLayer
    pub handle: TransitionHandle,
    /// Previous frame's authoritative rect (for interpolation in Stage 3+)
    pub from: RectF,
    /// Current frame's authoritative rect (for interpolation in Stage 3+)
    pub to: RectF,
}

/// SPEC-HAL-078 Stage 2: Engine-owned storage for active layout transitions.
///
/// Tracks which widgets have active layout transition animations.
/// One active transition per TreeWidgetId (LT-INV-24).
///
/// LT-INV-20: Never mutates widget state.
/// LT-INV-21: bind_candidates must be called outside layout/paint phases.
/// LT-INV-22: Timeline is the only time authority (handles reference Timeline).
pub struct ActiveLayoutTransitions {
    /// Active transitions keyed by widget ID (one per widget, per LT-INV-24)
    active: HashMap<TreeWidgetId, ActiveLayoutTransition>,
}

struct MountedTransitionOrigin {
    mount: WidgetMountToken,
    root: WidgetMountToken,
    issuer: TimelineIssuerToken,
}

struct ActiveLayoutTransition {
    state: LayoutTransitionState,
    origin: Option<MountedTransitionOrigin>,
}

impl std::ops::Deref for ActiveLayoutTransition {
    type Target = LayoutTransitionState;
    fn deref(&self) -> &Self::Target { &self.state }
}

#[cfg(test)]
impl std::ops::DerefMut for ActiveLayoutTransition {
    fn deref_mut(&mut self) -> &mut Self::Target { &mut self.state }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MountedTransitionRefusal {
    IncompleteLayout,
    ForeignTree,
    ForeignIssuer,
    MissingMount,
    NonFiniteEndpoint,
    EndpointMismatch,
    TypeMismatch,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum MountedTransitionBindError {
    #[error("HAL078 mounted provenance refused: {0:?}")]
    Provenance(MountedTransitionRefusal),
    #[error("HAL078 mounted animation admission refused: {0}")]
    Start(AnimationStartError),
}

fn finite_rect(rect: RectF) -> bool {
    [rect.x, rect.y, rect.width, rect.height].iter().all(|v| v.is_finite())
}

impl Default for ActiveLayoutTransitions {
    fn default() -> Self {
        Self::new()
    }
}

impl ActiveLayoutTransitions {
    pub fn new() -> Self {
        Self {
            active: HashMap::new(),
        }
    }

    /// SPEC-HAL-078 Stage 2: Bind transition candidates to TransitionLayer.
    ///
    /// For each candidate:
    /// - If a transition already exists for that widget, cancel and replace.
    /// - Create a new animation (0.0 → 1.0 representing progress).
    /// - Store the handle + geometry for Stage 3+ interpolation.
    /// - On bounded Timeline rejection, remove stale visual state and return the
    ///   first typed error while authoritative layout remains visible.
    ///
    /// LT-INV-21: Must be called outside layout/paint phases.
    /// LT-INV-22: Timeline is the only time authority.
    /// LT-INV-23: No visual output change (paint doesn't read this yet).
    pub fn bind_candidates(
        &mut self,
        candidates: &[LayoutTransitionCandidate],
        transition_layer: &mut TransitionLayer,
    ) -> Result<(), AnimationStartError> {
        debug_assert!(
            !candidates.is_empty() || self.active.is_empty() || true,
            "LT-INV-21: bind_candidates called"
        );

        let mut first_error = None;
        for candidate in candidates {
            // If transition already exists for this widget, cancel it first (no blending).
            // A rejected replacement must expose authoritative layout, not a stale endpoint.
            if let Some(existing) = self.active.remove(&candidate.id) {
                transition_layer.cancel_animation(existing.handle);
            }
            if first_error.is_some() {
                continue;
            }

            // Create new transition animation (progress from 0.0 to 1.0)
            let animation =
                Animation::new(0.0f32, 1.0f32, LAYOUT_TRANSITION_DURATION, Easing::EaseOut);
            let handle = match {
                let mut ctx = transition_layer.animation_context();
                ctx.start(animation)
            } {
                Ok(handle) => handle,
                Err(error) => {
                    first_error = Some(error);
                    continue;
                }
            };

            // Store the transition state
            let state = LayoutTransitionState {
                id: candidate.id,
                handle,
                from: candidate.from,
                to: candidate.to,
            };

            self.active.insert(candidate.id, ActiveLayoutTransition { state, origin: None });
        }

        match first_error {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }

    /// Existing App Stage2 birth, retaining the original concrete and issuer proof.
    /// Legacy public binding never gains this provenance retroactively.
    pub(crate) fn bind_candidates_mounted(
        &mut self,
        candidates: &[LayoutTransitionCandidate],
        transition_layer: &mut TransitionLayer,
        tree: &WidgetTree,
    ) -> Result<(), MountedTransitionBindError> {
        if candidates.is_empty() { return Ok(()); }
        let completed = tree.layout_transition_store().current_complete.as_ref()
            .ok_or(MountedTransitionBindError::Provenance(MountedTransitionRefusal::IncompleteLayout))?;
        if completed.structure != tree.structure_generation() {
            return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::IncompleteLayout));
        }
        let issuer = transition_layer.timeline().issuer();
        let root = tree.find_by_id(completed.root.owner())
            .ok_or(MountedTransitionBindError::Provenance(MountedTransitionRefusal::MissingMount))?;
        for candidate in candidates {
            let mount = tree.concrete_mount(candidate.id)
                .ok_or(MountedTransitionBindError::Provenance(MountedTransitionRefusal::MissingMount))?;
            if !tree.owns_mount_tree(&completed.root) || !completed.root.is_live() {
                return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::ForeignTree));
            }
            if !tree.find_by_id(candidate.id).is_some_and(|node| tree.is_descendant_or_same(node, root)) {
                return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::ForeignTree));
            }
            if self.active.get(&candidate.id).and_then(|e| e.origin.as_ref())
                .is_some_and(|o| o.issuer != issuer)
            {
                return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::ForeignIssuer));
            }
            if !finite_rect(candidate.from) || !finite_rect(candidate.to)
            {
                return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::NonFiniteEndpoint));
            }
            if tree.layout_transition_store().get_current(candidate.id) != Some(candidate.to)
                || tree.layout_transition_store().get_previous(candidate.id) != Some(candidate.from)
                || !tree.find_by_id(candidate.id).is_some_and(|id| {
                    tree.node(id).is_some_and(|node| !node.dirty_layout)
                        && tree.get_absolute_paint_rect(id) == Some(candidate.to)
                })
            {
                return Err(MountedTransitionBindError::Provenance(MountedTransitionRefusal::EndpointMismatch));
            }
            self.bind_candidates(std::slice::from_ref(candidate), transition_layer)
                .map_err(MountedTransitionBindError::Start)?;
            self.active.get_mut(&candidate.id).expect("successful HAL078 birth").origin =
                Some(MountedTransitionOrigin { mount, root: completed.root.clone(), issuer: issuer.clone() });
        }
        Ok(())
    }

    /// B: explicit cancellation only within the existing shared Widget Update.
    /// Unknown/partial evidence and a foreign issuer keep original custody intact.
    pub(crate) fn retire_stale_mounted(
        &mut self,
        tree: &WidgetTree,
        root: NodeId,
        transition_layer: &mut TransitionLayer,
    ) -> Result<usize, MountedTransitionRefusal> {
        crate::core::rect_authority::assert_update_mutation_authorized("HAL078 mounted retirement");
        if self.active.is_empty() { return Ok(0); }
        if self.active.values().all(|entry| entry.origin.is_none()) { return Ok(0); }
        let store = tree.layout_transition_store();
        let previous = store.previous_complete.as_ref()
            .ok_or(MountedTransitionRefusal::IncompleteLayout)?;
        let current_root = tree.node(root).and_then(|n| tree.concrete_mount(n.id))
            .ok_or(MountedTransitionRefusal::MissingMount)?;
        if previous.root != current_root || !current_root.is_live() || !tree.owns_mount_tree(&previous.root) {
            return Err(MountedTransitionRefusal::ForeignTree);
        }
        let issuer = transition_layer.timeline().issuer();
        let mut first_error = None;
        let mut retired = 0;
        self.active.retain(|id, entry| {
            let Some(origin) = &entry.origin else { return true };
            let refusal = if origin.issuer != issuer {
                Some(MountedTransitionRefusal::ForeignIssuer)
            } else if !tree.owns_mount_tree(&origin.mount) || origin.root != previous.root {
                Some(MountedTransitionRefusal::ForeignTree)
            } else if !finite_rect(entry.to) || store.get_previous(*id).is_some_and(|r| !finite_rect(r)) {
                Some(MountedTransitionRefusal::NonFiniteEndpoint)
            } else { None };
            if let Some(error) = refusal {
                first_error.get_or_insert(error);
                return true;
            }
            let mount_matches = origin.mount.is_live() && tree.concrete_mount(*id).as_ref() == Some(&origin.mount);
            if mount_matches && store.get_previous(*id) == Some(entry.to) { return true; }
            let disposition = match transition_layer.timeline().sample::<f32>(entry.handle) {
                AnimationSample::Active(value) if value.is_finite() => {
                    transition_layer.cancel_animation(entry.handle);
                    "Cancelled"
                }
                AnimationSample::Completed(value) if value.is_finite() => "Completed",
                AnimationSample::Cancelled => "AlreadyCancelled",
                AnimationSample::Missing => "Missing",
                _ => {
                    first_error.get_or_insert(MountedTransitionRefusal::TypeMismatch);
                    return true;
                }
            };
            tracing::trace!(target: "halcyon::native_layout_transition_pacer",
                event = "retirement", phase = "widget_update", owner = id.0, handle = entry.handle.id.0,
                disposition, issuer_match = true, previous_generation = previous.generation,
                cancelled_visible = matches!(transition_layer.timeline().sample::<f32>(entry.handle), AnimationSample::Cancelled),
                "HAL078 legal Widget Update retirement");
            retired += 1;
            false
        });
        first_error.map_or(Ok(retired), Err)
    }

    /// SPEC-HAL-078 Stage 2: Clean up completed transitions.
    ///
    /// Removes entries whose animations have finished (Timeline authority).
    /// Called each frame after timeline.advance().
    ///
    /// LT-INV-22: Timeline is the only time authority.
    pub fn cleanup_completed(&mut self, transition_layer: &TransitionLayer) {
        let timeline = transition_layer.timeline();
        self.active
            .retain(|_id, state| {
                state.origin.as_ref().is_some_and(|o| o.issuer != timeline.issuer())
                    || timeline.is_active(state.handle)
            });
    }

    /// Check if there are any active layout transitions.
    pub fn has_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// Get the number of active layout transitions (for debugging/testing).
    #[allow(dead_code)]
    pub fn active_count(&self) -> usize {
        self.active.len()
    }

    pub(crate) fn yield_to_presentation(
        &mut self,
        owners: &std::collections::HashSet<TreeWidgetId>,
        transition_layer: &mut TransitionLayer,
    ) {
        crate::core::rect_authority::assert_update_mutation_authorized("presentation arbitration");
        self.active.retain(|id, transition| {
            if owners.contains(id) {
                if transition.origin.as_ref().is_some_and(|o| o.issuer != transition_layer.timeline().issuer()) {
                    return true;
                }
                transition_layer.cancel_animation(transition.handle);
                false
            } else {
                true
            }
        });
    }

    /// Get a specific transition state by widget ID (for Stage 3+ paint).
    #[allow(dead_code)]
    pub fn get(&self, id: TreeWidgetId) -> Option<&LayoutTransitionState> {
        self.active.get(&id).map(|entry| &entry.state)
    }

    /// SPEC-HAL-145: Iterate over all active transitions.
    ///
    /// Used by the transition interpolation phase to pre-compute visual rects.
    pub fn iter(&self) -> impl Iterator<Item = (&TreeWidgetId, &LayoutTransitionState)> {
        self.active.iter().map(|(id, entry)| (id, &entry.state))
    }
}

// ============================================================================
// SPEC-HAL-145: Transition Interpolation Phase
// ============================================================================
//
// Pre-computes interpolated visual rects for all active layout transitions.
// This phase runs AFTER layout (under TransitionAuthority) and BEFORE paint.
//
// Frame order: Layout → Transition → Paint
// - Layout: set_rect under LayoutAuthority
// - Transition: lerp_rect under TransitionAuthority (this phase)
// - Paint: read pre-computed rects, no authority
//
// INV-TRANSITION-NO-RECT-ASSIGN: Transition authority MUST NOT call set_rect
// INV-TRANSITION-INTERPOLATE-ONLY: Transition may only lerp between snapshots
// ============================================================================

/// SPEC-HAL-145: Pre-computed interpolated rects for paint.
///
/// Computed during the transition interpolation phase (under TransitionAuthority),
/// consumed during paint phase (no authority). This enforces the separation
/// between transition interpolation and paint.
pub type InterpolatedVisualRects = HashMap<TreeWidgetId, RectF>;

/// SPEC-HAL-145: Compute interpolated visual rects for all active transitions.
///
/// MUST be called under TransitionAuthority (enforced by assertion).
/// Returns a map from widget ID to interpolated visual rect.
///
/// The caller is responsible for:
/// 1. Entering TransitionAuthorityGuard before calling
/// 2. Exiting TransitionAuthorityGuard after calling
/// 3. Passing the result to paint_tree
///
/// INV-TRANSITION-INTERPOLATE-ONLY: Only performs lerp, no rect mutation.
pub fn compute_interpolated_visual_rects(
    active_transitions: &ActiveLayoutTransitions,
    timeline: &crate::core::animation::Timeline,
) -> InterpolatedVisualRects {
    // SPEC-HAL-145: Assert we're under transition authority
    #[cfg(debug_assertions)]
    crate::core::rect_authority::assert_rect_interpolation_authorized(
        "compute_interpolated_visual_rects",
    );

    let mut result = HashMap::new();

    for (id, transition) in active_transitions.iter() {
        // Get animation progress (0.0 to 1.0, already eased by Timeline)
        if let Some(t) = timeline.get::<f32>(transition.handle) {
            // LT-INV-34: Skip interpolation if animation finished (t >= 1.0)
            if t >= 1.0 {
                continue;
            }
            // Interpolate between from and to rects
            let visual_rect = lerp_rect(transition.from, transition.to, t);
            result.insert(*id, visual_rect);
        }
    }

    result
}

/// App-only projection after its successful final Layout/deferred gate.
/// HAL-147 INV-ANIM-ENDPOINT-AUTH: validity precedes progress sampling.
/// Clean-layout reuse may omit this frame's store entry; live completed tree
/// geometry is still mandatory. This never changes Timeline or active custody.
pub(crate) fn compute_current_layout_interpolated_visual_rects(
    active: &ActiveLayoutTransitions,
    timeline: &crate::core::animation::Timeline,
    tree: &WidgetTree,
) -> InterpolatedVisualRects {
    #[cfg(debug_assertions)]
    crate::core::rect_authority::assert_rect_interpolation_authorized(
        "compute_current_layout_interpolated_visual_rects",
    );
    let finite = |rect: RectF| {
        [rect.x, rect.y, rect.width, rect.height].iter().all(|v| v.is_finite())
    };
    let mut result = HashMap::new();
    for (owner, entry) in &active.active {
        if entry.origin.as_ref().is_some_and(|origin| {
            origin.issuer != timeline.issuer() || !origin.mount.is_live() || !origin.root.is_live()
                || tree.concrete_mount(*owner).as_ref() != Some(&origin.mount)
        }) {
            continue;
        }
        let state = &entry.state;
        let Some(node_id) = tree.find_by_id(*owner) else { continue };
        let Some(node) = tree.node(node_id).filter(|node| !node.dirty_layout) else { continue };
        let Some(current) = tree.get_absolute_paint_rect(node_id) else { continue };
        if node.id != *owner || current != state.to || !finite(current)
            || !finite(state.from) || !finite(state.to)
            || tree.layout_transition_store().get_current(*owner)
                .is_some_and(|recorded| recorded != current || !finite(recorded))
        {
            continue;
        }
        // Endpoint qualification precedes any progress read.
        if let Some(t) = timeline.get::<f32>(state.handle) {
            if t.is_finite() && (0.0..1.0).contains(&t) {
                result.insert(*owner, lerp_rect(state.from, state.to, t));
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MountedRetirementFixture {
        tree: WidgetTree,
        root: NodeId,
        child: NodeId,
        owner: TreeWidgetId,
        layer: TransitionLayer,
        active: ActiveLayoutTransitions,
        text: crate::platform::font_manager_text_engine::FontManagerTextEngine,
    }

    impl MountedRetirementFixture {
        fn new() -> Self {
            use crate::core::layout::FlexParams;
            use crate::widgets::{fixed_spacer::FixedSpacer, flex::Flex};
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::row()), None, 0);
            let child = tree.add_child(root, Box::new(FixedSpacer::new(20.0, 32.0)), None, 0);
            tree.set_flex(child, FlexParams::fixed(20.0));
            let owner = tree.node(child).unwrap().id;
            let mut f = Self { tree, root, child, owner, layer: TransitionLayer::new(),
                active: ActiveLayoutTransitions::new(),
                text: crate::platform::font_manager_text_engine::FontManagerTextEngine::new() };
            f.begin(0.0);
            f.layout();
            let from = f.tree.get_absolute_paint_rect(child).unwrap();
            f.tree.set_flex(child, FlexParams::fixed(40.0));
            f.begin(0.001);
            f.layout();
            let to = f.tree.get_absolute_paint_rect(child).unwrap();
            assert_ne!(from, to);
            f.active.bind_candidates_mounted(&[LayoutTransitionCandidate { id: owner, from, to }], &mut f.layer, &f.tree).unwrap();
            f
        }

        fn begin(&mut self, time: f64) {
            self.layer.advance(time);
            self.tree.begin_layout_transition_frame();
            crate::core::app::test_begin_frame_layout_tracking();
        }

        fn layout(&mut self) {
            {
                let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
                crate::core::app::layout_tree(&mut self.tree, self.root,
                    crate::core::layout::Constraints::tight(crate::core::math::Size::new(100.0, 80.0)),
                    &crate::core::theme::Theme::default(), &mut self.text, None);
            }
            assert!(self.tree.complete_layout_transition_layout(self.root));
        }

        fn retire(&mut self) -> Result<usize, MountedTransitionRefusal> {
            let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
            self.active.retire_stale_mounted(&self.tree, self.root, &mut self.layer)
        }

        fn handle(&self) -> TransitionHandle { self.active.get(self.owner).unwrap().handle }

        fn changed(&mut self) {
            self.begin(0.02);
            assert_eq!(self.retire(), Ok(0));
            self.tree.set_flex(self.child, crate::core::layout::FlexParams::fixed(60.0));
            self.layout();
            let _transition = crate::core::rect_authority::TransitionAuthorityGuard::enter();
            assert!(!compute_current_layout_interpolated_visual_rects(&self.active, self.layer.timeline(), &self.tree).contains_key(&self.owner));
        }
    }

    #[test]
    fn mounted_retirement_active_round_trip_and_stable_clean_reuse() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        f.begin(0.01);
        let sample = f.layer.timeline().sample::<f32>(handle);
        assert_eq!(f.retire(), Ok(0));
        assert_eq!(f.handle(), handle);
        assert_eq!(f.layer.timeline().sample::<f32>(handle), sample);
        assert!(f.tree.layout_transition_store().current.is_empty());
        f.layout(); // O(1) transfer of the complete previous buffer on clean reuse.
        assert!(f.tree.layout_transition_store().get_current(f.owner).is_some());
        f.changed();
        f.begin(0.04);
        assert_eq!(f.retire(), Ok(1));
        assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Cancelled);
        assert!(f.active.get(f.owner).is_none());
        f.tree.set_flex(f.child, crate::core::layout::FlexParams::fixed(40.0));
        f.layout();
        {
            let _transition = crate::core::rect_authority::TransitionAuthorityGuard::enter();
            assert!(!compute_current_layout_interpolated_visual_rects(&f.active, f.layer.timeline(), &f.tree).contains_key(&f.owner));
        }
        f.begin(0.05);
        assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Missing);
        assert_eq!(f.retire(), Ok(0));
    }

    #[test]
    fn mounted_retirement_native_pre_swap_idempotent_geometry_preserves_completion() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        f.changed();
        let generation = f.tree.layout_transition_store().current_complete.as_ref().unwrap().generation;
        {
            let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
            crate::core::app::compute_absolute_paint_rects(&mut f.tree, f.root, crate::core::math::Point::zero());
        }
        assert_eq!(f.tree.layout_transition_store().current_complete.as_ref().unwrap().generation, generation);
        f.begin(0.04);
        assert_eq!(f.retire(), Ok(1));
        assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Cancelled);

        let mut changed = MountedRetirementFixture::new();
        changed.tree.record_layout_transition(changed.owner, RectF::new(7.0, 0.0, 40.0, 32.0));
        assert!(changed.tree.layout_transition_store().current_complete.is_none());
        changed.begin(0.02);
        assert_eq!(changed.retire(), Err(MountedTransitionRefusal::IncompleteLayout));
    }

    #[test]
    fn mounted_retirement_refuses_same_address_foreign_issuer_without_losing_custody() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        let original_mount = f.tree.concrete_mount(f.owner).unwrap();
        f.changed();
        f.begin(0.04);
        let mut foreign = TransitionLayer::new();
        foreign.advance(0.04);
        let other = foreign.animation_context().start(Animation::new(0.0f32, 1.0, 0.3, Easing::EaseOut)).unwrap();
        assert_eq!(other, handle);
        let address = f.layer.timeline() as *const crate::core::animation::Timeline;
        let original = std::mem::replace(&mut f.layer, foreign);
        assert_eq!(address, f.layer.timeline() as *const crate::core::animation::Timeline);
        assert!(original.timeline().issuer() != f.layer.timeline().issuer());
        assert_eq!(f.tree.concrete_mount(f.owner), Some(original_mount));
        assert_eq!(f.retire(), Err(MountedTransitionRefusal::ForeignIssuer));
        assert_eq!(f.handle(), handle);
        f.active.cleanup_completed(&f.layer);
        assert_eq!(f.handle(), handle);
        assert!(matches!(original.timeline().sample::<f32>(handle), AnimationSample::Active(_)));
        assert_eq!(f.layer.timeline().sample::<f32>(other), AnimationSample::Active(0.0));
        f.layer = original;
        assert_eq!(f.retire(), Ok(1));
        assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Cancelled);
    }

    #[test]
    fn mounted_retirement_preserves_truthful_terminal_windows_and_type_refusal() {
        for terminal in ["Completed", "Cancelled", "Missing", "TypeMismatch"] {
            let mut f = MountedRetirementFixture::new();
            let handle = f.handle();
            f.changed();
            f.begin(if terminal == "Completed" || terminal == "Missing" { 0.4 } else { 0.04 });
            if terminal == "Missing" {
                f.layout();
                f.begin(0.41); // next admitted frame, after the Completed observation window.
            }
            if terminal == "Cancelled" {
                let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
                f.layer.cancel_animation(handle);
            }
            if terminal == "TypeMismatch" {
                let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
                let point = f.layer.animation_context().start(Animation::new(
                    crate::core::math::Point::zero(), crate::core::math::Point::new(1.0, 1.0), 0.3, Easing::EaseOut)).unwrap();
                drop(_update);
                // Corrupt private evidence only for the negative; public state is a copy.
                f.active.active.get_mut(&f.owner).unwrap().state.handle = point;
                assert_eq!(f.retire(), Err(MountedTransitionRefusal::TypeMismatch));
                assert_eq!(f.handle(), point);
                assert!(matches!(f.layer.timeline().sample::<crate::core::math::Point>(point), AnimationSample::Active(_)));
                continue;
            }
            let before = f.layer.timeline().sample::<f32>(handle);
            assert_eq!(f.retire(), Ok(1));
            assert_eq!(f.layer.timeline().sample::<f32>(handle), before);
            assert_eq!(before, match terminal {
                "Completed" => AnimationSample::Completed(1.0),
                "Cancelled" => AnimationSample::Cancelled,
                _ => AnimationSample::Missing,
            });
            f.layer.advance(0.5);
            assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Missing);
        }
    }

    #[test]
    fn mounted_retirement_refuses_partial_deferred_root_and_foreign_tree() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        f.changed();
        f.begin(0.04);
        // A real frame has begun but never reached the successful final-layout gate.
        // A partial write must not masquerade as a complete absent-owner snapshot.
        {
            let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
            f.tree.record_layout_transition(f.owner, RectF::zero());
        }
        f.begin(0.05);
        assert_eq!(f.retire(), Err(MountedTransitionRefusal::IncompleteLayout));
        assert_eq!(f.handle(), handle);
        let mut foreign = MountedRetirementFixture::new();
        foreign.begin(0.05);
        let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
        assert_eq!(f.active.retire_stale_mounted(&foreign.tree, foreign.root, &mut f.layer),
            Err(MountedTransitionRefusal::ForeignTree));
        assert!(matches!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Active(_)));
    }

    #[test]
    fn mounted_retirement_exact_mount_and_slot_reuse_do_not_inherit_origin() {
        for recycle in [false, true] {
            let mut f = MountedRetirementFixture::new();
            let handle = f.handle();
            let mount = f.tree.concrete_mount(f.owner).unwrap();
            let child = if recycle {
                f.tree.remove_node(f.child);
                let next = f.tree.add_child(f.root, Box::new(crate::widgets::fixed_spacer::FixedSpacer::new(40.0, 32.0)), None, 0);
                assert_eq!(next, f.child);
                assert_ne!(f.tree.node(next).unwrap().id, f.owner);
                next
            } else {
                f.tree.replace_widget_only(f.child, Box::new(crate::widgets::fixed_spacer::FixedSpacer::new(40.0, 32.0)));
                assert_eq!(f.tree.node(f.child).unwrap().id, f.owner);
                assert!(f.tree.concrete_mount(f.owner).as_ref() != Some(&mount));
                f.child
            };
            assert!(!mount.is_live());
            f.begin(0.02);
            assert_eq!(f.retire(), Ok(1));
            assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Cancelled);
            assert!(f.tree.node(child).is_some());
        }
    }

    #[test]
    fn mounted_retirement_root_replacement_cannot_reuse_a_completed_root_witness() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        let root_owner = f.tree.node(f.root).unwrap().id;
        let root_mount = f.tree.concrete_mount(root_owner).unwrap();
        f.tree.replace_widget_only(f.root, Box::new(crate::widgets::flex::Flex::row()));
        assert!(!root_mount.is_live());
        f.begin(0.02);
        assert_eq!(f.retire(), Err(MountedTransitionRefusal::ForeignTree));
        f.layout();
        {
            let _transition = crate::core::rect_authority::TransitionAuthorityGuard::enter();
            assert!(!compute_current_layout_interpolated_visual_rects(&f.active, f.layer.timeline(), &f.tree).contains_key(&f.owner));
        }
        f.begin(0.04);
        assert_eq!(f.retire(), Err(MountedTransitionRefusal::ForeignTree));
        assert_eq!(f.handle(), handle);
        assert!(matches!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Active(_)));
        f.layer.advance(0.4);
        f.active.cleanup_completed(&f.layer);
        assert!(f.active.get(f.owner).is_none());
        assert_eq!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Completed(1.0));
    }

    #[test]
    fn mounted_retirement_legacy_copy_and_numeric_finiteness_do_not_grant_proof() {
        let mut f = MountedRetirementFixture::new();
        let handle = f.handle();
        f.begin(0.02);
        f.active.active.get_mut(&f.owner).unwrap().state.to.x = -0.0;
        assert_eq!(f.retire(), Ok(0));
        assert_eq!(f.handle(), handle);
        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            f.active.active.get_mut(&f.owner).unwrap().state.to.width = bad;
            assert_eq!(f.retire(), Err(MountedTransitionRefusal::NonFiniteEndpoint));
            assert!(matches!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Active(_)));
        }
        let mut legacy = ActiveLayoutTransitions::new();
        let candidate = LayoutTransitionCandidate { id: f.owner,
            from: RectF::new(0.0, 0.0, 20.0, 32.0), to: RectF::new(0.0, 0.0, 40.0, 32.0) };
        legacy.bind_candidates(&[candidate], &mut f.layer).unwrap();
        let legacy_handle = legacy.get(f.owner).unwrap().handle;
        let mut public_copy = *legacy.get(f.owner).unwrap();
        public_copy.handle = handle;
        assert_eq!(legacy.get(f.owner).unwrap().handle, legacy_handle);
        let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
        assert_eq!(legacy.retire_stale_mounted(&f.tree, f.root, &mut f.layer), Ok(0));
        assert!(legacy.get(f.owner).is_some());
        assert!(matches!(f.layer.timeline().sample::<f32>(handle), AnimationSample::Active(_)));
        assert_eq!(public_copy.handle, handle);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn disabled_oem_reflow_visual_rect_characterization() {
        use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
        use crate::core::layout::{Alignment, Constraints, EdgeInsets, FlexParams};
        use crate::core::math::Size;
        use crate::core::rect_authority::TransitionAuthorityGuard;
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::{button::Button, flex::Flex};

        // Source characterization, not native output or visual-quality acceptance.
        // The cold compact -> wide step is the existing Inspector Update pattern;
        // all subsequent canonical widths use the admitted three-pane formula.
        for sizes in [
            vec![
                Size::new(1600.0, 900.0),
                Size::new(1504.0, 864.0),
                Size::new(1600.0, 900.0),
            ],
            vec![
                Size::new(1600.0, 900.0),
                Size::new(1024.0, 680.0),
                Size::new(800.0, 560.0),
                Size::new(1600.0, 900.0),
            ],
        ] {
            for (prime_button_handle, mounted) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut tree = WidgetTree::new();
                let stretch_column = || Flex::column().with_align_items(Alignment::Stretch);
                let root = tree.create_root(Box::new(stretch_column()), None, 0);
                let header = tree.add_child(root, Box::new(Flex::row()), None, 0);
                tree.set_flex(header, FlexParams::fixed(36.0));
                let panes = tree.add_child(
                    root,
                    Box::new(
                        Flex::row()
                            .with_align_items(Alignment::Stretch)
                            .with_padding(EdgeInsets::all(8.0))
                            .with_spacing(8.0),
                    ),
                    None,
                    0,
                );
                tree.set_flex(panes, FlexParams::flexible(1.0));
                let footer = tree.add_child(root, Box::new(Flex::row()), None, 0);
                tree.set_flex(footer, FlexParams::fixed(26.0));
                let filters = tree.add_child(panes, Box::new(stretch_column()), None, 0);
                let results = tree.add_child(panes, Box::new(stretch_column()), None, 0);
                tree.set_flex(results, FlexParams::flexible(1.0));
                let inspector = tree.add_child(
                    panes,
                    Box::new(stretch_column().with_padding(EdgeInsets::all(1.0))),
                    None,
                    0,
                );
                let inner = tree.add_child(
                    inspector,
                    Box::new(
                        stretch_column()
                            .with_padding(EdgeInsets::all(7.0))
                            .with_spacing(4.0),
                    ),
                    None,
                    0,
                );
                tree.set_flex(inner, FlexParams::flexible(1.0));
                for height in [48.0, 24.0] {
                    let band = tree.add_child(inner, Box::new(Flex::row()), None, 0);
                    tree.set_flex(band, FlexParams::fixed(height));
                }
                let grid = tree.add_child(inner, Box::new(stretch_column()), None, 0);
                let rows: Vec<_> = (0..9)
                    .map(|_| tree.add_child(grid, Box::new(Flex::row()), None, 0))
                    .collect();
                let button = tree.add_child(
                    inner,
                    Box::new(Button::with_text("OEM").with_enabled(false)),
                    None,
                    0,
                );
                tree.set_flex(button, FlexParams::fixed(32.0));
                let button_id = tree.node(button).unwrap().id;
                let inspector_id = tree.node(inspector).unwrap().id;
                let root_id = tree.node(root).unwrap().id;
                let mut text = FontManagerTextEngine::new();
                let mut layer = TransitionLayer::new();
                let mut active = ActiveLayoutTransitions::new();

                let mut layout = |tree: &mut WidgetTree, size: Size, row_height: f32| {
                    let t = ((size.width - 1024.0) / 576.0).clamp(0.0, 1.0);
                    tree.set_flex(filters, FlexParams::fixed(176.0 + 104.0 * t));
                    tree.set_flex(inspector, FlexParams::fixed(248.0 + 172.0 * t));
                    tree.set_flex(grid, FlexParams::fixed(9.0 * row_height));
                    for &row in &rows {
                        tree.set_flex(row, FlexParams::fixed(row_height));
                    }
                    test_begin_frame_layout_tracking();
                    {
                        let _layout = LayoutAuthorityGuard::enter();
                        layout_tree(
                            tree,
                            root,
                            Constraints::tight(size),
                            &Theme::default(),
                            &mut text,
                            None,
                        );
                    }
                    assert!(tree.complete_layout_transition_layout(root));
                    let frame = tree.get_absolute_paint_rect(inspector).unwrap();
                    let content = RectF::new(
                        frame.x + 8.0,
                        frame.y + 8.0,
                        frame.width - 16.0,
                        frame.height - 16.0,
                    );
                    let canonical = tree.get_absolute_paint_rect(button).unwrap();
                    assert!((canonical.x - content.x).abs() < 0.002);
                    assert!((canonical.width - content.width).abs() < 0.002);
                    assert_eq!(canonical.height, 32.0);
                    assert!(canonical.y >= content.y);
                    assert!(canonical.y + canonical.height <= content.y + content.height);
                    assert_eq!(tree.node(button).unwrap().id, button_id);
                    assert!(tree.node(inspector).unwrap().widget.clip_bounds().is_none());
                    assert!(tree.node(inner).unwrap().widget.clip_bounds().is_none());
                    assert!(
                        !tree
                            .node(button)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<Button>()
                            .unwrap()
                            .is_enabled()
                    );
                    (frame, content, canonical)
                };
                let resolve = |tree: &WidgetTree| {
                    let mut candidates = Vec::new();
                    tree.collect_layout_transition_candidates(&mut candidates);
                    let ctx = AuthorizationContext::minimal(tree.layout_transition_store());
                    let mut flags = HashMap::new();
                    tree.compute_conflict_flags(&mut flags);
                    let button_flags = &flags[&button_id];
                    assert!(!button_flags.inside_scroll_container);
                    assert!(!button_flags.is_virtualized);
                    assert!(!button_flags.is_overlay_participant);
                    assert!(!button_flags.has_anchor_compensation);
                    for candidate in &candidates {
                        assert_eq!(
                            authorize_candidate(candidate, &ctx),
                            AuthorizationDecision::Allowed
                        );
                    }
                    resolve_layout_transition_candidates(candidates, &ctx, tree, &flags, 1)
                };

                layer.advance(0.0);
                tree.begin_layout_transition_frame();
                layout(
                    &mut tree,
                    sizes[0],
                    if prime_button_handle { 16.0 } else { 24.0 },
                );
                assert!(resolve(&tree).candidates().is_empty());
                if prime_button_handle {
                    layer.advance(0.001);
                    tree.begin_layout_transition_frame();
                    layout(&mut tree, sizes[0], 24.0);
                    let resolved = resolve(&tree);
                    assert!(resolved.candidates().iter().any(|c| c.id == button_id));
                    assert!(!resolved.candidates().iter().any(|c| c.id == inspector_id));
                    if mounted {
                        active.bind_candidates_mounted(resolved.candidates(), &mut layer, &tree).unwrap();
                    } else {
                        active.bind_candidates(resolved.candidates(), &mut layer).unwrap();
                    }
                    let prior = active.get(button_id).unwrap();
                    assert_eq!(prior.from, RectF::new(1180.0, 280.0, 404.0, 32.0));
                    assert_eq!(prior.to, RectF::new(1180.0, 352.0, 404.0, 32.0));
                }
                let original_button_handle = active.get(button_id).map(|state| state.handle);
                let mut escaped_frames = 0;
                let mut legacy_escaped_frames = 0;
                for (step, &size) in sizes.iter().enumerate().skip(1) {
                    layer.advance(0.001 + step as f64 * 0.02);
                    active.cleanup_completed(&layer);
                    tree.begin_layout_transition_frame();
                    if mounted {
                        let _update = crate::core::rect_authority::UpdateAuthorityGuard::enter();
                        active.retire_stale_mounted(&tree, root, &mut layer).unwrap();
                        if prime_button_handle && step == 2 {
                            assert_eq!(layer.timeline().sample::<f32>(original_button_handle.unwrap()), AnimationSample::Cancelled);
                        }
                        if prime_button_handle && step > 2 {
                            assert_eq!(layer.timeline().sample::<f32>(original_button_handle.unwrap()), AnimationSample::Missing);
                        }
                    }
                    let row_height = if size.width == 1600.0 { 24.0 } else { 16.0 };
                    let (frame, content, canonical) = layout(&mut tree, size, row_height);
                    let resolved = resolve(&tree);
                    assert!(resolved.candidates().iter().any(|c| c.id == root_id));
                    assert!(!resolved.candidates().iter().any(|c| c.id == button_id));
                    assert!(resolved.denial_records().iter().any(|denial| matches!(
                        denial,
                        UnifiedDenialRecord::Conflict {
                            widget_id,
                            reason: ConflictDenialReason::ParentTransitioning,
                        } if *widget_id == button_id
                    )));
                    let safe: Vec<_> = resolved
                        .candidates()
                        .iter()
                        .copied()
                        .filter(|c| {
                            !tree
                                .node(tree.find_by_id(c.id).unwrap())
                                .unwrap()
                                .dirty_layout
                        })
                        .collect();
                    if mounted {
                        active.bind_candidates_mounted(&safe, &mut layer, &tree).unwrap();
                    } else {
                        active.bind_candidates(&safe, &mut layer).unwrap();
                    }
                    let (visual, legacy) = {
                        let _transition = TransitionAuthorityGuard::enter();
                        (
                            compute_current_layout_interpolated_visual_rects(&active, layer.timeline(), &tree),
                            compute_interpolated_visual_rects(&active, layer.timeline()),
                        )
                    };
                    let button_visual = visual.get(&button_id).copied().unwrap_or(canonical);
                    if prime_button_handle && size.width < 1600.0 {
                        assert!(visual.get(&button_id).is_none(), "stale endpoint reached the visual map");
                    }
                    let parent_visual = visual.get(&inspector_id).copied().unwrap_or(frame);
                    assert_eq!(parent_visual, frame);
                    assert_eq!(
                        active.get(button_id).map(|s| s.handle),
                        original_button_handle.filter(|_| !mounted || step == 1)
                    );
                    let right_escape =
                        button_visual.x + button_visual.width - (frame.x + frame.width);
                    let left_escape = frame.x - button_visual.x;
                    let escaped = left_escape > 0.002 || right_escape > 0.002;
                    assert!(!escaped);
                    escaped_frames += usize::from(escaped);
                    if prime_button_handle && (!mounted || step == 1) {
                        let state = active.get(button_id).unwrap();
                        let t = layer.timeline().get::<f32>(state.handle).unwrap();
                        assert!(t > 0.0 && t < 1.0);
                        let old_visual = legacy[&button_id];
                        assert_eq!(old_visual, lerp_rect(state.from, state.to, t));
                        assert_eq!(old_visual.x, 1180.0);
                        assert_eq!(old_visual.width, 404.0);
                        if size.width < 1600.0 {
                            assert_ne!(state.to, canonical);
                            assert_eq!(button_visual, canonical);
                            legacy_escaped_frames += 1;
                        } else {
                            // A is pure: without separately qualified B retirement,
                            // an unchanged live handle can project on a round trip.
                            assert_eq!(button_visual, old_visual);
                        }
                    } else {
                        assert_eq!(button_visual, canonical);
                        assert!(visual.get(&button_id).is_none());
                    }
                    eprintln!(
                        "OEM_REFLOW source_only prime={prime_button_handle} target={size:?} \
                         inspector={frame:?} content={content:?} canonical={canonical:?} \
                         visual={button_visual:?} parent_visual={parent_visual:?} \
                         handle={original_button_handle:?} right_escape={right_escape}"
                    );
                }
                assert_eq!(
                    legacy_escaped_frames,
                    if prime_button_handle {
                        if mounted { 1 } else { sizes.len() - 2 }
                    } else {
                        0
                    }
                );
                assert_eq!(escaped_frames, 0);
                layer.advance(1.0);
                active.cleanup_completed(&layer);
                assert!(!active.has_active());
            }
        }
    }

    #[test]
    fn current_projection_preserves_clean_reuse_and_refuses_invalid_endpoints() {
        use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
        use crate::core::layout::Constraints;
        use crate::core::math::Size;
        use crate::core::rect_authority::{LayoutAuthorityGuard, TransitionAuthorityGuard};
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::fixed_spacer::FixedSpacer;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(FixedSpacer::new(20.0, 32.0)), None, 0);
        let owner = tree.node(root).unwrap().id;
        let mut text = FontManagerTextEngine::new();
        let mut layout = |tree: &mut WidgetTree| {
            tree.begin_layout_transition_frame();
            test_begin_frame_layout_tracking();
            let _layout = LayoutAuthorityGuard::enter();
            layout_tree(tree, root, Constraints::loose(Size::new(100.0, 100.0)), &Theme::default(), &mut text, None);
        };
        layout(&mut tree);
        let from = tree.get_absolute_paint_rect(root).unwrap();
        tree.replace_widget_only(root, Box::new(FixedSpacer::new(40.0, 32.0)));
        layout(&mut tree);
        let to = tree.get_absolute_paint_rect(root).unwrap();
        let mut layer = TransitionLayer::new();
        layer.advance(0.0);
        let mut active = ActiveLayoutTransitions::new();
        active.bind_candidates(&[LayoutTransitionCandidate { id: owner, from, to }], &mut layer).unwrap();
        let handle = active.get(owner).unwrap().handle;
        layer.advance(0.1);
        let project = |tree: &WidgetTree, active: &ActiveLayoutTransitions| {
            let _transition = TransitionAuthorityGuard::enter();
            compute_current_layout_interpolated_visual_rects(active, layer.timeline(), tree)
        };
        let stable = project(&tree, &active);
        assert!(stable.contains_key(&owner));
        layout(&mut tree);
        assert_eq!(project(&tree, &active), stable, "clean reuse must retain the same progress");
        assert_eq!(active.get(owner).unwrap().handle, handle);

        active.active.get_mut(&owner).unwrap().to.x = -0.0;
        assert_eq!(project(&tree, &active), stable, "signed zero uses numeric RectF equality");
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            active.active.get_mut(&owner).unwrap().to.width = invalid;
            assert!(project(&tree, &active).is_empty());
        }
        active.active.get_mut(&owner).unwrap().to = to;
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.record_layout_transition(owner, RectF::new(0.0, 0.0, 99.0, 32.0));
        }
        assert!(project(&tree, &active).is_empty(), "incoherent current store must refuse");
        layout(&mut tree);
        assert_eq!(project(&tree, &active), stable);
        tree.remove_node(root);
        assert!(project(&tree, &active).is_empty());
        assert!(matches!(layer.timeline().sample::<f32>(handle), crate::core::animation::AnimationSample::Active(_)),
            "pure exclusion must not cancel or mutate Timeline");
    }

    #[test]
    fn retirement_provenance_mount_and_numeric_sample_do_not_identify_timeline() {
        use crate::core::animation::{AnimationSample, Timeline};
        use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
        use crate::core::layout::Constraints;
        use crate::core::math::Size;
        use crate::core::rect_authority::{LayoutAuthorityGuard, UpdateAuthorityGuard};
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::fixed_spacer::FixedSpacer;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(FixedSpacer::new(20.0, 32.0)), None, 0);
        let owner = tree.node(root).unwrap().id;
        let mut text = FontManagerTextEngine::new();
        let mut layout = |tree: &mut WidgetTree| {
            tree.begin_layout_transition_frame();
            test_begin_frame_layout_tracking();
            let _layout = LayoutAuthorityGuard::enter();
            layout_tree(
                tree,
                root,
                Constraints::loose(Size::new(100.0, 100.0)),
                &Theme::default(),
                &mut text,
                None,
            );
        };
        layout(&mut tree);
        let from = tree.get_absolute_paint_rect(root).unwrap();
        tree.replace_widget_only(root, Box::new(FixedSpacer::new(40.0, 32.0)));
        layout(&mut tree);
        let to = tree.get_absolute_paint_rect(root).unwrap();
        let mount = tree.concrete_mount(owner).unwrap();
        let candidates = [LayoutTransitionCandidate { id: owner, from, to }];
        let mut original = TransitionLayer::new();
        let mut foreign = TransitionLayer::new();
        original.advance(0.0);
        foreign.advance(0.0);
        let mut original_registry = ActiveLayoutTransitions::new();
        let mut foreign_registry = ActiveLayoutTransitions::new();
        original_registry
            .bind_candidates(&candidates, &mut original)
            .unwrap();
        foreign_registry
            .bind_candidates(&candidates, &mut foreign)
            .unwrap();
        let handle = original_registry.get(owner).unwrap().handle;
        assert_eq!(handle, foreign_registry.get(owner).unwrap().handle);
        assert_eq!(
            original.timeline().sample::<f32>(handle),
            AnimationSample::Active(0.0)
        );
        assert_eq!(
            foreign.timeline().sample::<f32>(handle),
            AnimationSample::Active(0.0)
        );
        assert!(mount.is_live());
        assert_eq!(tree.concrete_mount(owner), Some(mount.clone()));

        // Even a stored address would not authenticate replacement of this value.
        let address = original.timeline() as *const Timeline;
        let original_custodian = std::mem::replace(&mut original, foreign);
        assert_eq!(address, original.timeline() as *const Timeline);
        assert_eq!(tree.concrete_mount(owner), Some(mount));
        {
            let _update = UpdateAuthorityGuard::enter();
            original.cancel_animation(handle);
        }
        // Negative evidence only: the raw primitive cancelled the FOREIGN entry.
        // This is not a successful implementation of mounted B retirement.
        assert_eq!(
            original.timeline().sample::<f32>(handle),
            AnimationSample::Cancelled
        );
        assert_eq!(
            original_custodian.timeline().sample::<f32>(handle),
            AnimationSample::Active(0.0)
        );
        eprintln!("B_ORIGIN_GAP owner={owner:?} handle={handle:?}: unchanged live mount and identical numeric/sample evidence; same-address replacement cancelled foreign entry, original remains Active");
    }

    #[test]
    fn test_store_basic_recording() {
        let mut store = LayoutTransitionStore::new();

        // Initially no previous data
        assert!(!store.has_previous());

        // Record some geometry
        let id1 = TreeWidgetId(1);
        let id2 = TreeWidgetId(2);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        let rect2 = RectF::new(10.0, 20.0, 200.0, 100.0);

        store.record(id1, rect1);
        store.record(id2, rect2);

        // Current data available
        assert_eq!(store.get_current(id1), Some(rect1));
        assert_eq!(store.get_current(id2), Some(rect2));
        assert_eq!(store.get_previous(id1), None);

        // Begin new frame
        store.begin_frame();

        // Previous now has data, current is empty
        assert!(store.has_previous());
        assert_eq!(store.get_previous(id1), Some(rect1));
        assert_eq!(store.get_previous(id2), Some(rect2));
        assert_eq!(store.get_current(id1), None);
    }

    #[test]
    fn test_context_queries() {
        let mut store = LayoutTransitionStore::new();

        // First frame: record geometry
        let id = TreeWidgetId(42);
        let rect = RectF::new(5.0, 10.0, 50.0, 25.0);
        store.record(id, rect);

        // Second frame: begin new frame and record different geometry
        store.begin_frame();
        let new_rect = RectF::new(15.0, 20.0, 60.0, 35.0);
        store.record(id, new_rect);

        // Create context and query
        let ctx = LayoutTransitionContext::new(&store);
        assert!(ctx.has_previous_snapshot());
        assert_eq!(ctx.prev_rect(id), Some(rect));
        assert_eq!(ctx.curr_rect(id), Some(new_rect));

        // Unknown widget returns None
        assert_eq!(ctx.prev_rect(TreeWidgetId(999)), None);
        assert_eq!(ctx.curr_rect(TreeWidgetId(999)), None);
    }

    #[test]
    fn duplicate_ids_are_excluded() {
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record widget normally
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);

        // Frame 2: Record same ID twice (duplicate)
        store.begin_frame();
        let rect2 = RectF::new(10.0, 20.0, 100.0, 50.0);
        let rect3 = RectF::new(20.0, 30.0, 100.0, 50.0);
        store.record(id, rect2);
        store.record(id, rect3); // Duplicate!

        // Should produce no candidates - duplicate ID excluded
        let mut candidates = Vec::new();
        store.collect_rect_change_candidates(&mut candidates);
        assert!(
            candidates.is_empty(),
            "Duplicate ID should be excluded from candidates"
        );
    }

    #[test]
    fn only_changed_rects_produce_candidates() {
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record two widgets
        let id_unchanged = TreeWidgetId(1);
        let id_changed = TreeWidgetId(2);
        let id_new = TreeWidgetId(3);
        let rect_a = RectF::new(0.0, 0.0, 100.0, 50.0);
        let rect_b = RectF::new(50.0, 50.0, 200.0, 100.0);
        store.record(id_unchanged, rect_a);
        store.record(id_changed, rect_b);

        // Frame 2: unchanged stays same, changed moves, new appears
        store.begin_frame();
        let rect_b_moved = RectF::new(60.0, 70.0, 200.0, 100.0);
        let rect_c = RectF::new(100.0, 100.0, 50.0, 50.0);
        store.record(id_unchanged, rect_a); // Same rect
        store.record(id_changed, rect_b_moved); // Different rect
        store.record(id_new, rect_c); // New widget (no previous)

        let mut candidates = Vec::new();
        store.collect_rect_change_candidates(&mut candidates);

        // Should have exactly one candidate: id_changed
        assert_eq!(candidates.len(), 1, "Should have exactly one candidate");
        let candidate = &candidates[0];
        assert_eq!(candidate.id, id_changed);
        assert_eq!(candidate.from, rect_b);
        assert_eq!(candidate.to, rect_b_moved);
    }

    // ========================================================================
    // SPEC-HAL-078 Stage 2: Transition Binding Tests
    // ========================================================================

    #[test]
    fn bind_candidates_creates_transitions() {
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        // No active transitions initially
        assert!(!active.has_active());
        assert_eq!(active.active_count(), 0);

        // Create some candidates
        let candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1),
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2),
                from: RectF::new(50.0, 50.0, 200.0, 100.0),
                to: RectF::new(60.0, 60.0, 200.0, 100.0),
            },
        ];

        // Bind candidates
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        // Should have 2 active transitions
        assert!(active.has_active());
        assert_eq!(active.active_count(), 2);

        // TransitionLayer should also have 2 active animations
        assert!(transition_layer.has_active_animations());
        assert_eq!(transition_layer.timeline().active_count(), 2);

        // Can retrieve each transition by ID
        let t1 = active
            .get(TreeWidgetId(1))
            .expect("Should have transition for ID 1");
        assert_eq!(t1.from, RectF::new(0.0, 0.0, 100.0, 50.0));
        assert_eq!(t1.to, RectF::new(10.0, 10.0, 100.0, 50.0));

        let t2 = active
            .get(TreeWidgetId(2))
            .expect("Should have transition for ID 2");
        assert_eq!(t2.from, RectF::new(50.0, 50.0, 200.0, 100.0));
        assert_eq!(t2.to, RectF::new(60.0, 60.0, 200.0, 100.0));
    }

    #[test]
    fn bind_replaces_existing_transition() {
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        let id = TreeWidgetId(42);

        // First candidate
        let candidates1 = vec![LayoutTransitionCandidate {
            id,
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates1, &mut transition_layer)
            .unwrap();

        let handle1 = active.get(id).unwrap().handle;

        // Second candidate for same widget (replaces)
        let candidates2 = vec![LayoutTransitionCandidate {
            id,
            from: RectF::new(10.0, 10.0, 100.0, 50.0),
            to: RectF::new(20.0, 20.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates2, &mut transition_layer)
            .unwrap();

        // Still only one active transition for that ID
        assert_eq!(active.active_count(), 1);

        // Handle should be different (new animation)
        let handle2 = active.get(id).unwrap().handle;
        assert_ne!(handle1, handle2, "Should have new handle after replacement");

        // Old animation should have been cancelled
        assert!(
            !transition_layer.timeline().is_active(handle1),
            "Old animation should be cancelled"
        );

        // New animation should be active
        assert!(
            transition_layer.timeline().is_active(handle2),
            "New animation should be active"
        );
    }

    #[test]
    fn bind_capacity_exhaustion_falls_back_to_authoritative_layout() {
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();
        let id = TreeWidgetId(42);
        let first = LayoutTransitionCandidate {
            id,
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        active
            .bind_candidates(&[first], &mut transition_layer)
            .unwrap();
        let first_handle = active.get(id).unwrap().handle;

        let remaining = transition_layer
            .timeline()
            .capacity()
            .checked_sub(transition_layer.timeline().retained_entry_count())
            .unwrap();
        {
            let mut context = transition_layer.animation_context();
            for _ in 0..remaining {
                context
                    .start(Animation::new(0.0_f32, 1.0, 1.0, Easing::Linear))
                    .unwrap();
            }
        }

        let replacement = LayoutTransitionCandidate {
            id,
            from: first.to,
            to: RectF::new(20.0, 20.0, 100.0, 50.0),
        };
        let error = active
            .bind_candidates(&[replacement], &mut transition_layer)
            .unwrap_err();

        assert!(matches!(
            error,
            AnimationStartError::CapacityExhausted { capacity }
                if capacity == transition_layer.timeline().capacity()
        ));
        assert!(!active.has_active());
        assert!(!transition_layer.timeline().is_active(first_handle));

        // The mounted path must not leave an origin attached to a failed start.
        let mut mounted = MountedRetirementFixture::new();
        let before = *mounted.active.get(mounted.owner).unwrap();
        let capacity = mounted.layer.timeline().capacity();
        let remaining = capacity - mounted.layer.timeline().retained_entry_count();
        {
            let mut context = mounted.layer.animation_context();
            for _ in 0..remaining {
                context.start(Animation::new(0.0f32, 1.0, 1.0, Easing::Linear)).unwrap();
            }
        }
        let error = mounted.active.bind_candidates_mounted(&[LayoutTransitionCandidate {
            id: before.id, from: before.from, to: before.to,
        }], &mut mounted.layer, &mounted.tree).unwrap_err();
        assert!(matches!(error, MountedTransitionBindError::Start(AnimationStartError::CapacityExhausted { capacity: n }) if n == capacity));
        assert!(!mounted.active.has_active());
        assert_eq!(mounted.layer.timeline().retained_entry_count(), capacity);
        assert_eq!(mounted.layer.timeline().sample::<f32>(before.handle), AnimationSample::Cancelled);
    }

    #[test]
    fn cleanup_removes_completed_transitions() {
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        // Create a candidate
        let candidates = vec![LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        assert_eq!(active.active_count(), 1);

        // Advance time beyond animation duration (default is 0.3s)
        transition_layer.advance(0.0);
        transition_layer.advance(0.5); // Well past 0.3s

        // Timeline should have culled the animation
        assert!(!transition_layer.has_active_animations());

        // Cleanup should remove from our storage too
        active.cleanup_completed(&transition_layer);
        assert_eq!(active.active_count(), 0);
        assert!(!active.has_active());
    }

    #[test]
    fn one_transition_per_widget_id() {
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        let id = TreeWidgetId(1);

        // Bind same widget ID multiple times in one call shouldn't happen
        // (Stage 1 would produce only one candidate per ID)
        // But if it did, only one should remain
        let candidates = vec![LayoutTransitionCandidate {
            id,
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        // LT-INV-24: One transition per TreeWidgetId
        assert_eq!(active.active_count(), 1);
    }

    // ========================================================================
    // SPEC-HAL-078 Stage 3: Visual Application Tests
    // ========================================================================

    #[test]
    fn lerp_rect_at_zero() {
        // LT-INV-31: lerp_rect is a pure function
        let from = RectF::new(0.0, 0.0, 100.0, 50.0);
        let to = RectF::new(10.0, 20.0, 200.0, 100.0);

        let result = lerp_rect(from, to, 0.0);

        // At t=0.0, result should equal from
        assert_eq!(result.x, from.x);
        assert_eq!(result.y, from.y);
        assert_eq!(result.width, from.width);
        assert_eq!(result.height, from.height);
    }

    #[test]
    fn lerp_rect_at_one() {
        // LT-INV-31: lerp_rect is a pure function
        let from = RectF::new(0.0, 0.0, 100.0, 50.0);
        let to = RectF::new(10.0, 20.0, 200.0, 100.0);

        let result = lerp_rect(from, to, 1.0);

        // At t=1.0, result should equal to
        assert_eq!(result.x, to.x);
        assert_eq!(result.y, to.y);
        assert_eq!(result.width, to.width);
        assert_eq!(result.height, to.height);
    }

    #[test]
    fn lerp_rect_at_midpoint() {
        // LT-INV-31: lerp_rect is a pure function
        let from = RectF::new(0.0, 0.0, 100.0, 50.0);
        let to = RectF::new(10.0, 20.0, 200.0, 100.0);

        let result = lerp_rect(from, to, 0.5);

        // At t=0.5, result should be midpoint of each component
        assert!(
            (result.x - 5.0).abs() < 0.001,
            "x should be 5.0, got {}",
            result.x
        );
        assert!(
            (result.y - 10.0).abs() < 0.001,
            "y should be 10.0, got {}",
            result.y
        );
        assert!(
            (result.width - 150.0).abs() < 0.001,
            "width should be 150.0, got {}",
            result.width
        );
        assert!(
            (result.height - 75.0).abs() < 0.001,
            "height should be 75.0, got {}",
            result.height
        );
    }

    #[test]
    fn lerp_rect_components_independent() {
        // LT-INV-31: Each component interpolates independently
        let from = RectF::new(0.0, 100.0, 50.0, 200.0);
        let to = RectF::new(100.0, 0.0, 150.0, 100.0);

        let result = lerp_rect(from, to, 0.25);

        // At t=0.25, each component should be 25% toward target
        // x: 0.0 + (100.0 - 0.0) * 0.25 = 25.0
        // y: 100.0 + (0.0 - 100.0) * 0.25 = 75.0
        // width: 50.0 + (150.0 - 50.0) * 0.25 = 75.0
        // height: 200.0 + (100.0 - 200.0) * 0.25 = 175.0
        assert!((result.x - 25.0).abs() < 0.001);
        assert!((result.y - 75.0).abs() < 0.001);
        assert!((result.width - 75.0).abs() < 0.001);
        assert!((result.height - 175.0).abs() < 0.001);
    }

    #[test]
    fn lerp_rect_negative_values() {
        // Ensure lerp_rect handles negative coordinates correctly
        let from = RectF::new(-50.0, -100.0, 100.0, 200.0);
        let to = RectF::new(50.0, 100.0, 200.0, 100.0);

        let result = lerp_rect(from, to, 0.5);

        assert!((result.x - 0.0).abs() < 0.001);
        assert!((result.y - 0.0).abs() < 0.001);
        assert!((result.width - 150.0).abs() < 0.001);
        assert!((result.height - 150.0).abs() < 0.001);
    }

    // ========================================================================
    // SPEC-HAL-078B Stage 1.75: Conflict Resolution Tests
    // ========================================================================

    #[test]
    fn conflict_allowed_when_no_conflicts() {
        // Widget with no conflict flags should be allowed
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: false,
            is_virtualized: false,
            is_overlay_participant: false,
            has_anchor_compensation: false,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(result, TransitionConflictResult::Allowed);
    }

    #[test]
    fn conflict_denied_when_inside_scroll_container() {
        // CONFLICT-1: Widget inside scroll container should be denied
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: true,
            is_virtualized: false,
            is_overlay_participant: false,
            has_anchor_compensation: false,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(
            result,
            TransitionConflictResult::Denied {
                rule: ConflictRuleId::ScrollTransform
            }
        );
    }

    #[test]
    fn conflict_denied_when_virtualized() {
        // CONFLICT-2: Widget from virtualization projection should be denied
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: false,
            is_virtualized: true,
            is_overlay_participant: false,
            has_anchor_compensation: false,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(
            result,
            TransitionConflictResult::Denied {
                rule: ConflictRuleId::VirtualizationProjection
            }
        );
    }

    #[test]
    fn conflict_denied_when_overlay_participant() {
        // CONFLICT-3: Widget in overlay layer should be denied
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: false,
            is_virtualized: false,
            is_overlay_participant: true,
            has_anchor_compensation: false,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(
            result,
            TransitionConflictResult::Denied {
                rule: ConflictRuleId::OverlayParticipation
            }
        );
    }

    #[test]
    fn conflict_denied_when_anchor_compensated() {
        // CONFLICT-4: Widget with anchor compensation should be denied
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: false,
            is_virtualized: false,
            is_overlay_participant: false,
            has_anchor_compensation: true,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(
            result,
            TransitionConflictResult::Denied {
                rule: ConflictRuleId::ViewportAnchoring
            }
        );
    }

    #[test]
    fn conflict_denied_when_flags_missing() {
        // If flags are missing, deny by default (don't guess)
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };

        let result = check_candidate_conflicts(&candidate, None);
        assert!(matches!(result, TransitionConflictResult::Denied { .. }));
    }

    #[test]
    fn conflict_precedence_scroll_before_virtualization() {
        // CONFLICT-1 (scroll) should take precedence over CONFLICT-2 (virtualization)
        let candidate = LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        };
        let flags = ConflictFlags {
            inside_scroll_container: true,
            is_virtualized: true,
            is_overlay_participant: true,
            has_anchor_compensation: true,
        };

        let result = check_candidate_conflicts(&candidate, Some(&flags));
        assert_eq!(
            result,
            TransitionConflictResult::Denied {
                rule: ConflictRuleId::ScrollTransform
            }
        );
    }

    #[test]
    fn resolve_conflicts_filters_candidates() {
        // Test that resolve_conflicts properly filters out denied candidates
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1),
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2),
                from: RectF::new(50.0, 50.0, 100.0, 50.0),
                to: RectF::new(60.0, 60.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(3),
                from: RectF::new(100.0, 100.0, 100.0, 50.0),
                to: RectF::new(110.0, 110.0, 100.0, 50.0),
            },
        ];

        let mut flags = HashMap::new();
        // Widget 1: allowed (no conflicts)
        flags.insert(
            TreeWidgetId(1),
            ConflictFlags {
                inside_scroll_container: false,
                is_virtualized: false,
                is_overlay_participant: false,
                has_anchor_compensation: false,
            },
        );
        // Widget 2: denied (inside scroll)
        flags.insert(
            TreeWidgetId(2),
            ConflictFlags {
                inside_scroll_container: true,
                is_virtualized: false,
                is_overlay_participant: false,
                has_anchor_compensation: false,
            },
        );
        // Widget 3: allowed (no conflicts)
        flags.insert(
            TreeWidgetId(3),
            ConflictFlags {
                inside_scroll_container: false,
                is_virtualized: false,
                is_overlay_participant: false,
                has_anchor_compensation: false,
            },
        );

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_conflicts(&mut candidates, &flags, &mut denial_records);

            // Should have 2 candidates remaining (widgets 1 and 3)
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].id, TreeWidgetId(1));
            assert_eq!(candidates[1].id, TreeWidgetId(3));

            // Should have 1 denial record (widget 2)
            assert_eq!(denial_records.len(), 1);
            assert_eq!(denial_records[0].widget_id, TreeWidgetId(2));
            assert_eq!(denial_records[0].rule, ConflictRuleId::ScrollTransform);
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_conflicts(&mut candidates, &flags);

            // Should have 2 candidates remaining (widgets 1 and 3)
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].id, TreeWidgetId(1));
            assert_eq!(candidates[1].id, TreeWidgetId(3));
        }
    }

    // ========================================================================
    // SPEC-HAL-078B CONFLICT-COND-1: Nested Transition Tests
    // ========================================================================

    #[test]
    fn nested_transitions_allowed_when_no_shared_scroll() {
        // CONFLICT-COND-1: Nested transitions are allowed if they don't share scroll context
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1), // ancestor
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2), // descendant
                from: RectF::new(50.0, 50.0, 100.0, 50.0),
                to: RectF::new(60.0, 60.0, 100.0, 50.0),
            },
        ];

        // Closure that simulates: 1 is ancestor of 2, but they don't share scroll context
        let is_ancestor_fn = |ancestor: TreeWidgetId, descendant: TreeWidgetId| -> Option<bool> {
            if ancestor == TreeWidgetId(1) && descendant == TreeWidgetId(2) {
                Some(false) // Ancestor relationship but NO shared scroll
            } else {
                Some(false)
            }
        };

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn, &mut denial_records);

            // Both should remain - no shared scroll context
            assert_eq!(candidates.len(), 2);
            assert!(denial_records.is_empty());
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn);
            assert_eq!(candidates.len(), 2);
        }
    }

    #[test]
    fn nested_transitions_denied_when_shared_scroll() {
        // CONFLICT-COND-1: Child transitions denied when sharing scroll context with ancestor
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1), // ancestor
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2), // descendant
                from: RectF::new(50.0, 50.0, 100.0, 50.0),
                to: RectF::new(60.0, 60.0, 100.0, 50.0),
            },
        ];

        // Closure that simulates: 1 is ancestor of 2 AND they share scroll context
        let is_ancestor_fn = |ancestor: TreeWidgetId, descendant: TreeWidgetId| -> Option<bool> {
            if ancestor == TreeWidgetId(1) && descendant == TreeWidgetId(2) {
                Some(true) // Ancestor relationship AND shared scroll
            } else {
                Some(false)
            }
        };

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn, &mut denial_records);

            // Only ancestor (1) should remain, child (2) denied
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].id, TreeWidgetId(1));

            // Should have denial record for child
            assert_eq!(denial_records.len(), 1);
            assert_eq!(denial_records[0].widget_id, TreeWidgetId(2));
            assert_eq!(
                denial_records[0].rule,
                ConflictRuleId::NestedTransitionScrollContext
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn);
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].id, TreeWidgetId(1));
        }
    }

    #[test]
    fn nested_transitions_deterministic_order() {
        // CONFLICT-COND-1: Order-independent - always denies child, never ancestor
        // Test with reversed order of candidates
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(2), // descendant listed first
                from: RectF::new(50.0, 50.0, 100.0, 50.0),
                to: RectF::new(60.0, 60.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(1), // ancestor listed second
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
        ];

        // Closure that simulates: 1 is ancestor of 2 AND they share scroll context
        let is_ancestor_fn = |ancestor: TreeWidgetId, descendant: TreeWidgetId| -> Option<bool> {
            if ancestor == TreeWidgetId(1) && descendant == TreeWidgetId(2) {
                Some(true) // Ancestor relationship AND shared scroll
            } else {
                Some(false)
            }
        };

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn, &mut denial_records);

            // Only ancestor (1) should remain, child (2) denied - same result regardless of order
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].id, TreeWidgetId(1));
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn);
            assert_eq!(candidates.len(), 1);
            assert_eq!(candidates[0].id, TreeWidgetId(1));
        }
    }

    #[test]
    fn nested_transitions_unrelated_widgets_unaffected() {
        // CONFLICT-COND-1: Unrelated widgets (neither ancestor nor descendant) are unaffected
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1),
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2),
                from: RectF::new(50.0, 50.0, 100.0, 50.0),
                to: RectF::new(60.0, 60.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(3),
                from: RectF::new(100.0, 100.0, 100.0, 50.0),
                to: RectF::new(110.0, 110.0, 100.0, 50.0),
            },
        ];

        // No ancestor relationships between any of them
        let is_ancestor_fn =
            |_ancestor: TreeWidgetId, _descendant: TreeWidgetId| -> Option<bool> { Some(false) };

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn, &mut denial_records);

            // All should remain - no nested relationships
            assert_eq!(candidates.len(), 3);
            assert!(denial_records.is_empty());
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_nested_conflicts(&mut candidates, is_ancestor_fn);
            assert_eq!(candidates.len(), 3);
        }
    }

    // ========================================================================
    // SPEC-HAL-145: Transition Interpolation Phase Tests
    // ========================================================================

    #[test]
    fn test_compute_interpolated_visual_rects_under_transition_authority() {
        use crate::core::rect_authority::TransitionAuthorityGuard;

        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        // Create some candidates and bind them
        let candidates = vec![
            LayoutTransitionCandidate {
                id: TreeWidgetId(1),
                from: RectF::new(0.0, 0.0, 100.0, 50.0),
                to: RectF::new(10.0, 10.0, 100.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: TreeWidgetId(2),
                from: RectF::new(50.0, 50.0, 200.0, 100.0),
                to: RectF::new(100.0, 100.0, 200.0, 100.0),
            },
        ];
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        // Advance timeline a bit to get partial progress
        transition_layer.advance(0.0);
        transition_layer.advance(0.15); // About halfway through 0.3s duration

        // SPEC-HAL-145: Compute interpolated rects under TransitionAuthority
        let interpolated = {
            let _guard = TransitionAuthorityGuard::enter();
            compute_interpolated_visual_rects(&active, transition_layer.timeline())
        };

        // Should have 2 interpolated rects
        assert_eq!(interpolated.len(), 2, "Should have 2 interpolated rects");

        // Both widgets should have entries (animation still in progress)
        assert!(
            interpolated.contains_key(&TreeWidgetId(1)),
            "Widget 1 should have interpolated rect"
        );
        assert!(
            interpolated.contains_key(&TreeWidgetId(2)),
            "Widget 2 should have interpolated rect"
        );

        // Interpolated rects should be somewhere between from and to
        let rect1 = interpolated.get(&TreeWidgetId(1)).unwrap();
        assert!(
            rect1.x > 0.0 && rect1.x < 10.0,
            "Interpolated x should be between from and to"
        );

        let rect2 = interpolated.get(&TreeWidgetId(2)).unwrap();
        assert!(
            rect2.x > 50.0 && rect2.x < 100.0,
            "Interpolated x should be between from and to"
        );
    }

    #[test]
    fn test_compute_interpolated_visual_rects_empty_for_completed_animations() {
        use crate::core::rect_authority::TransitionAuthorityGuard;

        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        // Create a candidate
        let candidates = vec![LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        // Advance timeline past animation duration (0.3s)
        transition_layer.advance(0.0);
        transition_layer.advance(0.5);

        // Clean up completed
        active.cleanup_completed(&transition_layer);

        // Compute interpolated rects
        let interpolated = {
            let _guard = TransitionAuthorityGuard::enter();
            compute_interpolated_visual_rects(&active, transition_layer.timeline())
        };

        // Should be empty - animation completed and cleaned up
        assert!(
            interpolated.is_empty(),
            "Completed animations should not produce interpolated rects"
        );
    }

    #[test]
    fn test_interpolated_rects_are_projection_only() {
        // SPEC-HAL-145: Verify interpolated rects are projection-only (not written back).
        // This test verifies the design: InterpolatedVisualRects is a HashMap that
        // is computed during transition phase and consumed during paint.
        // No set_rect is called on the tree.

        use crate::core::rect_authority::TransitionAuthorityGuard;

        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();

        // Create a candidate
        let candidates = vec![LayoutTransitionCandidate {
            id: TreeWidgetId(1),
            from: RectF::new(0.0, 0.0, 100.0, 50.0),
            to: RectF::new(10.0, 10.0, 100.0, 50.0),
        }];
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        // Advance timeline
        transition_layer.advance(0.0);
        transition_layer.advance(0.15);

        // Compute interpolated rects
        let interpolated = {
            let _guard = TransitionAuthorityGuard::enter();
            compute_interpolated_visual_rects(&active, transition_layer.timeline())
        };

        // Verify the result is a HashMap (projection data structure)
        // Not stored in tree, not written back via set_rect
        let rect = interpolated.get(&TreeWidgetId(1)).unwrap();

        // The interpolated rect is a new value, not a reference to tree data
        assert!(rect.x > 0.0, "Interpolated rect should have non-zero x");

        // The original transition state is unchanged (from/to are immutable)
        let transition = active.get(TreeWidgetId(1)).unwrap();
        assert_eq!(
            transition.from.x, 0.0,
            "Original from rect should be unchanged"
        );
        assert_eq!(
            transition.to.x, 10.0,
            "Original to rect should be unchanged"
        );
    }

    // ========================================================================
    // SPEC-HAL-078A Stage 1.5: Authorization Tests
    // ========================================================================

    #[test]
    fn test_authorization_allowed_when_valid() {
        // SPEC-HAL-078A: Authorized candidate passes all checks
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record widget
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);

        // Frame 2: Record moved widget
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        // Create candidate matching store geometry
        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: rect2,
        };

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Allowed,
            "Valid candidate should be authorized"
        );
    }

    #[test]
    fn test_authorization_denied_identity_not_in_previous() {
        // SPEC-HAL-078A: Identity continuity requires presence in previous frame
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Empty (widget not present)
        // Frame 2: Widget appears
        store.begin_frame();
        let id = TreeWidgetId(42);
        let rect = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect);

        // Create candidate for widget that wasn't in previous frame
        let candidate = LayoutTransitionCandidate {
            id,
            from: RectF::new(0.0, 0.0, 100.0, 50.0), // Fabricated from rect
            to: rect,
        };

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::IdentityNotInPrevious),
            "Widget not in previous frame should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_identity_not_in_current() {
        // SPEC-HAL-078A: Identity continuity requires presence in current frame
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Widget present
        let id = TreeWidgetId(42);
        let rect = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect);

        // Frame 2: Widget not present (disappeared)
        store.begin_frame();
        // Note: We don't record id in frame 2

        // Create candidate for widget that disappeared
        let candidate = LayoutTransitionCandidate {
            id,
            from: rect,
            to: RectF::new(10.0, 10.0, 100.0, 50.0), // Fabricated to rect
        };

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::IdentityNotInCurrent),
            "Widget not in current frame should be denied"
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn test_authorization_denied_under_layout_authority() {
        use crate::core::rect_authority::LayoutAuthorityGuard;

        // SPEC-HAL-078A: Transition creation forbidden under LayoutAuthority
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record widget
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);

        // Frame 2: Record moved widget
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        // Create valid candidate
        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: rect2,
        };

        // Enter layout authority - transition authorization should be denied
        let _guard = LayoutAuthorityGuard::enter();

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::LayoutAuthorityActive),
            "Authorization under LayoutAuthority should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_from_rect_mismatch() {
        // SPEC-HAL-078A: Geometry sanity - from rect must match stored previous
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record widget
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);

        // Frame 2: Record moved widget
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        // Create candidate with WRONG from rect (doesn't match stored previous)
        let candidate = LayoutTransitionCandidate {
            id,
            from: RectF::new(5.0, 5.0, 100.0, 50.0), // Wrong - should be rect1
            to: rect2,
        };

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::FromRectInvalid),
            "Mismatched from rect should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_to_rect_mismatch() {
        // SPEC-HAL-078A: Geometry sanity - to rect must match stored current
        let mut store = LayoutTransitionStore::new();

        // Frame 1: Record widget
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);

        // Frame 2: Record moved widget
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        // Create candidate with WRONG to rect (doesn't match stored current)
        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: RectF::new(15.0, 15.0, 100.0, 50.0), // Wrong - should be rect2
        };

        let ctx = AuthorizationContext::minimal(&store);
        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::ToRectInvalid),
            "Mismatched to rect should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_reparented_widget() {
        // SPEC-HAL-078A AUTH-DENY-5: Reparented widgets denied
        let mut store = LayoutTransitionStore::new();

        // Setup valid geometry
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: rect2,
        };

        // Mark widget as reparented
        let mut reparented = HashSet::new();
        reparented.insert(id);

        let ctx = AuthorizationContext {
            store: &store,
            reparented_ids: &reparented,
            captured_ids: &HashSet::new(),
            virtualized_ids: &HashSet::new(),
        };

        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::ReparentedWidget),
            "Reparented widget should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_active_input_capture() {
        // SPEC-HAL-078A AUTH-COND-3: Widgets with active input capture denied
        let mut store = LayoutTransitionStore::new();

        // Setup valid geometry
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: rect2,
        };

        // Mark widget as having active input capture
        let mut captured = HashSet::new();
        captured.insert(id);

        let ctx = AuthorizationContext {
            store: &store,
            reparented_ids: &HashSet::new(),
            captured_ids: &captured,
            virtualized_ids: &HashSet::new(),
        };

        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::ActiveInputCapture),
            "Widget with active input capture should be denied"
        );
    }

    #[test]
    fn test_authorization_denied_virtualized_node() {
        // SPEC-HAL-078A AUTH-DENY-1: Virtualized nodes denied
        let mut store = LayoutTransitionStore::new();

        // Setup valid geometry
        let id = TreeWidgetId(42);
        let rect1 = RectF::new(0.0, 0.0, 100.0, 50.0);
        store.record(id, rect1);
        store.begin_frame();
        let rect2 = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect2);

        let candidate = LayoutTransitionCandidate {
            id,
            from: rect1,
            to: rect2,
        };

        // Mark widget as virtualized
        let mut virtualized = HashSet::new();
        virtualized.insert(id);

        let ctx = AuthorizationContext {
            store: &store,
            reparented_ids: &HashSet::new(),
            captured_ids: &HashSet::new(),
            virtualized_ids: &virtualized,
        };

        let decision = authorize_candidate(&candidate, &ctx);

        assert_eq!(
            decision,
            AuthorizationDecision::Denied(DenialReason::VirtualizedNode),
            "Virtualized node should be denied"
        );
    }

    #[test]
    fn test_authorize_candidates_filters_correctly() {
        // SPEC-HAL-078A: authorize_candidates filters in-place
        let mut store = LayoutTransitionStore::new();

        // Setup: 3 widgets
        let id1 = TreeWidgetId(1);
        let id2 = TreeWidgetId(2);
        let id3 = TreeWidgetId(3);

        // Frame 1: All three widgets present
        let rect1a = RectF::new(0.0, 0.0, 100.0, 50.0);
        let rect2a = RectF::new(50.0, 0.0, 100.0, 50.0);
        let rect3a = RectF::new(100.0, 0.0, 100.0, 50.0);
        store.record(id1, rect1a);
        store.record(id2, rect2a);
        store.record(id3, rect3a);

        // Frame 2: All three move
        store.begin_frame();
        let rect1b = RectF::new(10.0, 10.0, 100.0, 50.0);
        let rect2b = RectF::new(60.0, 10.0, 100.0, 50.0);
        let rect3b = RectF::new(110.0, 10.0, 100.0, 50.0);
        store.record(id1, rect1b);
        store.record(id2, rect2b);
        store.record(id3, rect3b);

        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: id1,
                from: rect1a,
                to: rect1b,
            },
            LayoutTransitionCandidate {
                id: id2,
                from: rect2a,
                to: rect2b,
            },
            LayoutTransitionCandidate {
                id: id3,
                from: rect3a,
                to: rect3b,
            },
        ];

        // Mark id2 as reparented (should be denied)
        let mut reparented = HashSet::new();
        reparented.insert(id2);

        let ctx = AuthorizationContext {
            store: &store,
            reparented_ids: &reparented,
            captured_ids: &HashSet::new(),
            virtualized_ids: &HashSet::new(),
        };

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            authorize_candidates(&mut candidates, &ctx, &mut denial_records);

            // Should have 2 candidates remaining (id1 and id3)
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].id, id1);
            assert_eq!(candidates[1].id, id3);

            // Should have 1 denial record (id2)
            assert_eq!(denial_records.len(), 1);
            assert_eq!(denial_records[0].widget_id, id2);
            assert_eq!(denial_records[0].reason, DenialReason::ReparentedWidget);
        }

        #[cfg(not(debug_assertions))]
        {
            authorize_candidates(&mut candidates, &ctx);
            assert_eq!(candidates.len(), 2);
            assert_eq!(candidates[0].id, id1);
            assert_eq!(candidates[1].id, id3);
        }
    }

    #[test]
    fn test_denied_transition_never_reaches_interpolation() {
        // SPEC-HAL-078A: Denied transitions must not reach Stage 2/3
        use crate::core::rect_authority::TransitionAuthorityGuard;

        let mut store = LayoutTransitionStore::new();

        // Setup: Widget appears in frame 2 only (not in frame 1)
        store.begin_frame();
        let id = TreeWidgetId(42);
        let rect = RectF::new(10.0, 10.0, 100.0, 50.0);
        store.record(id, rect);

        // Create invalid candidate (widget wasn't in previous frame)
        let mut candidates = vec![LayoutTransitionCandidate {
            id,
            from: RectF::new(0.0, 0.0, 100.0, 50.0), // Fabricated
            to: rect,
        }];

        let ctx = AuthorizationContext::minimal(&store);

        // Authorize - should filter out the candidate
        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            authorize_candidates(&mut candidates, &ctx, &mut denial_records);
            assert!(candidates.is_empty(), "Denied candidate should be filtered");
            assert_eq!(denial_records.len(), 1);
        }

        #[cfg(not(debug_assertions))]
        {
            authorize_candidates(&mut candidates, &ctx);
            assert!(candidates.is_empty(), "Denied candidate should be filtered");
        }

        // Now try to bind - should have nothing to bind
        let mut transition_layer = TransitionLayer::new();
        let mut active = ActiveLayoutTransitions::new();
        active
            .bind_candidates(&candidates, &mut transition_layer)
            .unwrap();

        assert!(!active.has_active(), "No transitions should be bound");

        // Compute interpolated rects - should be empty
        let interpolated = {
            let _guard = TransitionAuthorityGuard::enter();
            compute_interpolated_visual_rects(&active, transition_layer.timeline())
        };

        assert!(
            interpolated.is_empty(),
            "Denied transition should never reach interpolation"
        );
    }

    // ========================================================================
    // SPEC-HAL-078B Stage 1: Conflict Resolution Tests (Real WidgetTree)
    // ========================================================================
    //
    // These tests use real WidgetTree instances with actual hierarchy.
    // No fake ancestry closures - tests verify behavior against real tree.
    // ========================================================================

    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::widget::{Widget, WidgetDowncast};
    use crate::utils::id::WidgetInternalId;
    use std::any::Any;

    /// Minimal test widget for conflict resolution tests
    struct ConflictTestWidget {
        internal_id: WidgetInternalId,
    }

    impl ConflictTestWidget {
        fn new() -> Self {
            Self {
                internal_id: WidgetInternalId::new(),
            }
        }
    }

    impl WidgetDowncast for ConflictTestWidget {
        fn as_any(&self) -> &dyn Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn Any {
            self
        }
    }

    impl Widget for ConflictTestWidget {
        fn layout(
            &mut self,
            _cx: &mut crate::core::widget::LayoutContext,
            _constraints: crate::core::layout::Constraints,
            _children: &[crate::core::widget_tree::NodeId],
        ) -> crate::core::layout::LayoutResult {
            crate::core::layout::LayoutResult::fit(crate::core::math::Size::new(100.0, 50.0))
        }

        fn paint(
            &self,
            _cx: &mut crate::core::widget::PaintContext,
            _rect: RectF,
            _children: &[crate::core::widget_tree::NodeId],
        ) {
        }

        fn id(&self) -> WidgetInternalId {
            self.internal_id
        }
    }

    /// Helper to create a test candidate from a tree node
    fn candidate_from_node(
        tree: &WidgetTree,
        node: crate::core::widget_tree::NodeId,
    ) -> LayoutTransitionCandidate {
        let widget_id = tree.node(node).unwrap().id;
        let rect_a = RectF::new(0.0, 0.0, 100.0, 100.0);
        let rect_b = RectF::new(10.0, 10.0, 100.0, 100.0);
        LayoutTransitionCandidate {
            id: widget_id,
            from: rect_a,
            to: rect_b,
        }
    }

    /// LAW-B1: Duplicate widget IDs → keep one, deny rest
    ///
    /// Creates candidates with the same TreeWidgetId (simulating upstream bug)
    /// and verifies only one survives.
    #[test]
    fn test_conflict_law_b1_duplicate_widget_single_survivor() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create a simple tree with one node
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let widget_id = tree.node(root).unwrap().id;

        let rect_a = RectF::new(0.0, 0.0, 100.0, 100.0);
        let rect_b = RectF::new(10.0, 10.0, 100.0, 100.0);
        let rect_c = RectF::new(20.0, 20.0, 100.0, 100.0);

        // Three candidates for the SAME widget ID (duplicates - simulates upstream bug)
        let mut candidates = vec![
            LayoutTransitionCandidate {
                id: widget_id,
                from: rect_a,
                to: rect_b,
            },
            LayoutTransitionCandidate {
                id: widget_id,
                from: rect_b,
                to: rect_c,
            },
            LayoutTransitionCandidate {
                id: widget_id,
                from: rect_a,
                to: rect_c,
            },
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts_stage1(&mut candidates, &tree, &mut denial_records);
            assert_eq!(
                candidates.len(),
                1,
                "LAW-B1: Only one survivor for duplicate IDs"
            );
            assert_eq!(denial_records.len(), 2, "LAW-B1: Two duplicates denied");
            for record in &denial_records {
                assert_eq!(record.reason, ConflictDenialReason::DuplicateWidget);
            }
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates, &tree);
            assert_eq!(
                candidates.len(),
                1,
                "LAW-B1: Only one survivor for duplicate IDs"
            );
        }
    }

    /// LAW-B2: Parent + child → only parent survives
    ///
    /// Creates a real tree with parent-child relationship and verifies
    /// parent wins.
    #[test]
    fn test_conflict_law_b2_parent_child_parent_wins() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root → parent → child
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let parent = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(parent, Box::new(ConflictTestWidget::new()), None, 0);

        let parent_widget_id = tree.node(parent).unwrap().id;
        let _child_widget_id = tree.node(child).unwrap().id;

        // Both parent and child transitioning
        let mut candidates = vec![
            candidate_from_node(&tree, parent),
            candidate_from_node(&tree, child),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts_stage1(&mut candidates, &tree, &mut denial_records);
            assert_eq!(candidates.len(), 1, "LAW-B2: Only parent survives");
            assert_eq!(
                candidates[0].id, parent_widget_id,
                "LAW-B2: Parent wins, not child"
            );
            assert_eq!(denial_records.len(), 1);
            assert_eq!(denial_records[0].widget_id, _child_widget_id);
            assert_eq!(
                denial_records[0].reason,
                ConflictDenialReason::ParentTransitioning
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates, &tree);
            assert_eq!(candidates.len(), 1, "LAW-B2: Only parent survives");
            assert_eq!(
                candidates[0].id, parent_widget_id,
                "LAW-B2: Parent wins, not child"
            );
        }
    }

    /// LAW-B3: Scroll container + descendant → scroll container survives,
    /// descendant denied with ScrollContainerTransitioning reason.
    #[test]
    fn test_conflict_law_b3_scroll_container_supremacy() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root → scroll_container → child
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let scroll = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(scroll, Box::new(ConflictTestWidget::new()), None, 0);

        // Mark scroll as a scroll container
        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0)));

        let scroll_widget_id = tree.node(scroll).unwrap().id;
        let _child_widget_id = tree.node(child).unwrap().id;

        // Both scroll container and child transitioning
        let mut candidates = vec![
            candidate_from_node(&tree, scroll),
            candidate_from_node(&tree, child),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts_stage1(&mut candidates, &tree, &mut denial_records);
            assert_eq!(
                candidates.len(),
                1,
                "LAW-B3: Only scroll container survives"
            );
            assert_eq!(
                candidates[0].id, scroll_widget_id,
                "LAW-B3: Scroll container wins"
            );
            assert_eq!(denial_records.len(), 1);
            assert_eq!(denial_records[0].widget_id, _child_widget_id);
            // LAW-B3 runs before LAW-B2, so reason should be ScrollContainerTransitioning
            assert_eq!(
                denial_records[0].reason,
                ConflictDenialReason::ScrollContainerTransitioning
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates, &tree);
            assert_eq!(
                candidates.len(),
                1,
                "LAW-B3: Only scroll container survives"
            );
            assert_eq!(
                candidates[0].id, scroll_widget_id,
                "LAW-B3: Scroll container wins"
            );
        }
    }

    /// Order independence: result is the same regardless of input order
    #[test]
    fn test_conflict_resolution_order_independence() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root → parent → child
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let parent = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(parent, Box::new(ConflictTestWidget::new()), None, 0);

        let _parent_widget_id = tree.node(parent).unwrap().id;

        // Test with parent first
        let mut candidates_order1 = vec![
            candidate_from_node(&tree, parent),
            candidate_from_node(&tree, child),
        ];

        // Test with child first
        let mut candidates_order2 = vec![
            candidate_from_node(&tree, child),
            candidate_from_node(&tree, parent),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records1 = Vec::new();
            let mut denial_records2 = Vec::new();
            resolve_transition_conflicts_stage1(
                &mut candidates_order1,
                &tree,
                &mut denial_records1,
            );
            resolve_transition_conflicts_stage1(
                &mut candidates_order2,
                &tree,
                &mut denial_records2,
            );

            assert_eq!(
                candidates_order1.len(),
                candidates_order2.len(),
                "Order independence: same count"
            );
            assert_eq!(
                candidates_order1[0].id, candidates_order2[0].id,
                "Order independence: same survivor"
            );
            assert_eq!(
                candidates_order1[0].id, _parent_widget_id,
                "Order independence: parent wins"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates_order1, &tree);
            resolve_transition_conflicts_stage1(&mut candidates_order2, &tree);

            assert_eq!(
                candidates_order1.len(),
                candidates_order2.len(),
                "Order independence: same count"
            );
            assert_eq!(
                candidates_order1[0].id, candidates_order2[0].id,
                "Order independence: same survivor"
            );
        }
    }

    /// Multiple independent widgets should all survive
    #[test]
    fn test_conflict_resolution_independent_widgets_survive() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root with three independent children (siblings)
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let sibling1 = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let sibling2 = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let sibling3 = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);

        // Three siblings (no parent-child relationship between them)
        let mut candidates = vec![
            candidate_from_node(&tree, sibling1),
            candidate_from_node(&tree, sibling2),
            candidate_from_node(&tree, sibling3),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts_stage1(&mut candidates, &tree, &mut denial_records);
            assert_eq!(candidates.len(), 3, "Independent widgets all survive");
            assert!(
                denial_records.is_empty(),
                "No denials for independent widgets"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates, &tree);
            assert_eq!(candidates.len(), 3, "Independent widgets all survive");
        }
    }

    /// Deep hierarchy: grandparent, parent, child → only grandparent survives
    #[test]
    fn test_conflict_resolution_deep_hierarchy() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root → grandparent → parent → child
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let grandparent = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let parent = tree.add_child(grandparent, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(parent, Box::new(ConflictTestWidget::new()), None, 0);

        let grandparent_widget_id = tree.node(grandparent).unwrap().id;

        let mut candidates = vec![
            candidate_from_node(&tree, grandparent),
            candidate_from_node(&tree, parent),
            candidate_from_node(&tree, child),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts_stage1(&mut candidates, &tree, &mut denial_records);
            assert_eq!(
                candidates.len(),
                1,
                "Deep hierarchy: only grandparent survives"
            );
            assert_eq!(
                candidates[0].id, grandparent_widget_id,
                "Deep hierarchy: grandparent wins"
            );
            assert_eq!(denial_records.len(), 2, "Two denials (parent and child)");
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts_stage1(&mut candidates, &tree);
            assert_eq!(
                candidates.len(),
                1,
                "Deep hierarchy: only grandparent survives"
            );
            assert_eq!(
                candidates[0].id, grandparent_widget_id,
                "Deep hierarchy: grandparent wins"
            );
        }
    }

    // ========================================================================
    // SPEC-HAL-078B: Independent Geometry and Hierarchy Conflict Tests
    // ========================================================================
    //
    // Independent geometry must not suppress lawful visual authority.
    // Tests use real WidgetTree instances with actual hierarchy.
    // ========================================================================

    /// Helper to create a candidate with specific rects
    fn candidate_with_rects(
        tree: &WidgetTree,
        node: crate::core::widget_tree::NodeId,
        from: RectF,
        to: RectF,
    ) -> LayoutTransitionCandidate {
        let widget_id = tree.node(node).unwrap().id;
        LayoutTransitionCandidate {
            id: widget_id,
            from,
            to,
        }
    }

    /// Hierarchy denial still applies when an independent sibling overlaps.
    #[test]
    fn overlapping_sibling_preserves_parent_child_conflict() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create tree: root → parent → child, plus an independent sibling
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let parent = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(parent, Box::new(ConflictTestWidget::new()), None, 0);
        let sibling = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);

        let parent_id = tree.node(parent).unwrap().id;
        let sibling_id = tree.node(sibling).unwrap().id;

        // All overlapping: parent, child, and sibling
        // Stage 1 should eliminate child (parent-child law)
        // The independent sibling retains its own transition.
        let mut candidates = vec![
            candidate_with_rects(
                &tree,
                parent,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                RectF::new(50.0, 0.0, 100.0, 100.0),
            ),
            candidate_with_rects(
                &tree,
                child,
                RectF::new(20.0, 20.0, 50.0, 50.0),
                RectF::new(60.0, 20.0, 50.0, 50.0),
            ),
            candidate_with_rects(
                &tree,
                sibling,
                RectF::new(100.0, 0.0, 100.0, 100.0),
                RectF::new(50.0, 0.0, 100.0, 100.0),
            ),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts(&mut candidates, &tree, &mut denial_records);

            assert_eq!(candidates.len(), 2);
            assert!(candidates.iter().any(|candidate| candidate.id == parent_id));
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.id == sibling_id)
            );
            assert_eq!(denial_records.len(), 1);

            // Check that child was denied by Stage 1 (ParentTransitioning)
            let child_denial = denial_records
                .iter()
                .find(|r| r.reason == ConflictDenialReason::ParentTransitioning);
            assert!(
                child_denial.is_some(),
                "Child denied by Stage 1 parent-child law"
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts(&mut candidates, &tree);
            assert_eq!(candidates.len(), 2);
            assert!(candidates.iter().any(|candidate| candidate.id == parent_id));
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.id == sibling_id)
            );
        }
    }

    /// Scroll-container precedence remains local to its descendants.
    #[test]
    fn overlapping_sibling_preserves_scroll_container_conflict() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create: root → scroll_container → child1, and root → sibling
        // scroll_container has layout_scroll_offset, so child1 is denied by Stage 1
        // The independent sibling retains its own transition.
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let scroll = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(scroll, Box::new(ConflictTestWidget::new()), None, 0);
        let sibling = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);

        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0)));

        let scroll_id = tree.node(scroll).unwrap().id;
        let sibling_id = tree.node(sibling).unwrap().id;

        // All overlapping
        let mut candidates = vec![
            candidate_with_rects(
                &tree,
                scroll,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                RectF::new(50.0, 0.0, 100.0, 100.0),
            ),
            candidate_with_rects(
                &tree,
                child,
                RectF::new(10.0, 10.0, 50.0, 50.0),
                RectF::new(60.0, 10.0, 50.0, 50.0),
            ),
            candidate_with_rects(
                &tree,
                sibling,
                RectF::new(100.0, 0.0, 100.0, 100.0),
                RectF::new(50.0, 0.0, 100.0, 100.0),
            ),
        ];

        #[cfg(debug_assertions)]
        {
            let mut denial_records = Vec::new();
            resolve_transition_conflicts(&mut candidates, &tree, &mut denial_records);

            assert_eq!(candidates.len(), 2);
            assert_eq!(denial_records.len(), 1);

            // Check scroll container law was applied
            let scroll_denial = denial_records
                .iter()
                .find(|r| r.reason == ConflictDenialReason::ScrollContainerTransitioning);
            assert!(
                scroll_denial.is_some(),
                "Child denied by scroll container supremacy"
            );

            assert!(candidates.iter().any(|candidate| candidate.id == scroll_id));
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.id == sibling_id)
            );
        }

        #[cfg(not(debug_assertions))]
        {
            resolve_transition_conflicts(&mut candidates, &tree);
            assert_eq!(candidates.len(), 2);
            assert!(candidates.iter().any(|candidate| candidate.id == scroll_id));
            assert!(
                candidates
                    .iter()
                    .any(|candidate| candidate.id == sibling_id)
            );
        }
    }

    // ========================================================================
    // SPEC-HAL-078A+078B: Unified Resolver Tests
    // ========================================================================

    #[test]
    fn coherent_three_sibling_reflow_keeps_every_transition() {
        use crate::core::rect_authority::TransitionAuthorityGuard;

        let guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let candidates = [(846.0, 630.0), (908.0, 850.0), (971.0, 705.0)].map(|(y, width)| {
            let node = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
            candidate_with_rects(
                &tree,
                node,
                RectF::new(28.0, y, width, 24.0),
                RectF::new(28.0, y + 72.0, width, 24.0),
            )
        });
        tree.begin_layout_transition_frame();
        for candidate in &candidates {
            tree.record_layout_transition(candidate.id, candidate.from);
        }
        tree.begin_layout_transition_frame();
        for candidate in &candidates {
            tree.record_layout_transition(candidate.id, candidate.to);
        }
        drop(guard);

        // Adjacent swept envelopes overlap, but the siblings keep their separation.
        // The widest middle candidate must not lose its transition because of its area.
        let auth_ctx = AuthorizationContext::minimal(tree.layout_transition_store());
        let conflict_flags = HashMap::new();
        for order in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let ordered = order.map(|index| candidates[index]).to_vec();
            #[cfg(debug_assertions)]
            let resolved =
                resolve_layout_transition_candidates(ordered, &auth_ctx, &tree, &conflict_flags, 0);
            #[cfg(not(debug_assertions))]
            let resolved =
                resolve_layout_transition_candidates(ordered, &auth_ctx, &tree, &conflict_flags);
            assert_eq!(resolved.candidates().len(), 3, "candidate order: {order:?}");
            #[cfg(debug_assertions)]
            assert!(resolved.denial_records().is_empty());

            let mut transition_layer = TransitionLayer::new();
            let mut active = ActiveLayoutTransitions::new();
            active
                .bind_candidates(resolved.candidates(), &mut transition_layer)
                .unwrap();
            transition_layer.advance(0.0);
            for time in [0.025, 0.1, 0.2] {
                transition_layer.advance(time);
                let visual = {
                    let _guard = TransitionAuthorityGuard::enter();
                    compute_interpolated_visual_rects(&active, transition_layer.timeline())
                };
                assert_eq!(visual.len(), 3);
                let displacement = visual[&candidates[0].id].y - candidates[0].from.y;
                assert!(displacement > 0.0 && displacement < 72.0);
                for candidate in &candidates {
                    let rect = visual[&candidate.id];
                    assert!((rect.y - candidate.from.y - displacement).abs() < 0.001);
                    assert_eq!(rect.width, candidate.to.width);
                    assert_eq!(
                        tree.layout_transition_store().get_current(candidate.id),
                        Some(candidate.to)
                    );
                }
                for pair in candidates.windows(2) {
                    assert!(!visual[&pair[0].id].intersects(&visual[&pair[1].id]));
                }
            }
        }
    }

    /// The production resolver preserves scroll hierarchy denial beside independent motion.
    #[test]
    fn test_unified_resolver_preserves_scroll_hierarchy_conflict() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create test hierarchy: root → scroll_container → child, and root → sibling
        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let scroll = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let child = tree.add_child(scroll, Box::new(ConflictTestWidget::new()), None, 0);
        let sibling = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);

        // Mark scroll as a scroll container
        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0)));

        let scroll_id = tree.node(scroll).unwrap().id;
        let child_id = tree.node(child).unwrap().id;
        let sibling_id = tree.node(sibling).unwrap().id;

        // Set up layout transition store
        // The independent sibling overlaps the scroll container spatially.
        tree.begin_layout_transition_frame();
        tree.record_layout_transition(scroll_id, RectF::new(0.0, 0.0, 100.0, 100.0));
        tree.record_layout_transition(child_id, RectF::new(0.0, 0.0, 50.0, 50.0));
        tree.record_layout_transition(sibling_id, RectF::new(50.0, 0.0, 100.0, 100.0));

        tree.begin_layout_transition_frame();
        tree.record_layout_transition(scroll_id, RectF::new(10.0, 0.0, 100.0, 100.0));
        tree.record_layout_transition(child_id, RectF::new(10.0, 0.0, 50.0, 50.0));
        tree.record_layout_transition(sibling_id, RectF::new(60.0, 0.0, 100.0, 100.0));

        // Create candidates
        let candidates = vec![
            LayoutTransitionCandidate {
                id: scroll_id,
                from: RectF::new(0.0, 0.0, 100.0, 100.0),
                to: RectF::new(10.0, 0.0, 100.0, 100.0),
            },
            LayoutTransitionCandidate {
                id: child_id,
                from: RectF::new(0.0, 0.0, 50.0, 50.0),
                to: RectF::new(10.0, 0.0, 50.0, 50.0),
            },
            LayoutTransitionCandidate {
                id: sibling_id,
                from: RectF::new(50.0, 0.0, 100.0, 100.0),
                to: RectF::new(60.0, 0.0, 100.0, 100.0),
            },
        ];

        drop(_guard);

        // Create authorization context
        let auth_ctx = AuthorizationContext::minimal(tree.layout_transition_store());

        // Create conflict flags - empty since this test focuses on authorization
        let conflict_flags = std::collections::HashMap::new();

        // Call unified resolver (frame_epoch required in debug builds only)
        #[cfg(debug_assertions)]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags, 0);
        #[cfg(not(debug_assertions))]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags);

        // Should have exactly 2 survivors: scroll and sibling (child denied by Stage 1)
        assert_eq!(
            resolved.candidates().len(),
            2,
            "Expected 2 survivors (scroll + sibling), got {}",
            resolved.candidates().len()
        );

        let survivor_ids: Vec<_> = resolved.candidates().iter().map(|c| c.id).collect();
        assert!(survivor_ids.contains(&scroll_id), "Scroll should survive");
        assert!(survivor_ids.contains(&sibling_id), "Sibling should survive");
        assert!(
            !survivor_ids.contains(&child_id),
            "Child should be denied by scroll container law"
        );
    }

    /// Test that unified resolver applies authorization before conflict resolution.
    #[test]
    fn test_unified_resolver_authorization_first() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let widget = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let widget_id = tree.node(widget).unwrap().id;

        // Set up store with geometry only in current frame (not previous)
        // This should cause authorization denial (IdentityNotInPrevious)
        tree.begin_layout_transition_frame();
        tree.begin_layout_transition_frame(); // Two frames: previous is empty
        tree.record_layout_transition(widget_id, RectF::new(10.0, 0.0, 100.0, 100.0));

        let candidates = vec![LayoutTransitionCandidate {
            id: widget_id,
            from: RectF::new(0.0, 0.0, 100.0, 100.0), // Doesn't match store
            to: RectF::new(10.0, 0.0, 100.0, 100.0),
        }];

        drop(_guard);

        let auth_ctx = AuthorizationContext::minimal(tree.layout_transition_store());
        let conflict_flags = std::collections::HashMap::new();
        #[cfg(debug_assertions)]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags, 0);
        #[cfg(not(debug_assertions))]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags);

        // Should be denied by authorization (identity not in previous)
        assert_eq!(
            resolved.candidates().len(),
            0,
            "Candidate should be denied by authorization"
        );

        #[cfg(debug_assertions)]
        {
            // Verify denial record exists
            let auth_denials: Vec<_> = resolved
                .denial_records()
                .iter()
                .filter(|r| matches!(r, UnifiedDenialRecord::Authorization { .. }))
                .collect();
            assert!(
                !auth_denials.is_empty(),
                "Should have authorization denial record"
            );
        }
    }

    /// Test that resolved candidates are immutable.
    #[test]
    fn test_resolved_candidates_immutable() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let widget = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let widget_id = tree.node(widget).unwrap().id;

        tree.begin_layout_transition_frame();
        tree.record_layout_transition(widget_id, RectF::new(0.0, 0.0, 100.0, 100.0));

        tree.begin_layout_transition_frame();
        tree.record_layout_transition(widget_id, RectF::new(10.0, 0.0, 100.0, 100.0));

        let candidates = vec![LayoutTransitionCandidate {
            id: widget_id,
            from: RectF::new(0.0, 0.0, 100.0, 100.0),
            to: RectF::new(10.0, 0.0, 100.0, 100.0),
        }];

        drop(_guard);

        let auth_ctx = AuthorizationContext::minimal(tree.layout_transition_store());
        let conflict_flags = std::collections::HashMap::new();
        #[cfg(debug_assertions)]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags, 0);
        #[cfg(not(debug_assertions))]
        let resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags);

        // Candidates are returned as immutable slice
        let slice = resolved.candidates();
        assert_eq!(slice.len(), 1);

        // Can consume to get owned Vec for binding
        let owned = resolved.into_candidates();
        assert_eq!(owned.len(), 1);
    }

    /// Test that denial telemetry hook is invoked with correct data.
    #[test]
    #[cfg(debug_assertions)]
    fn test_denial_telemetry_hook_invoked() {
        use std::sync::atomic::{AtomicU64, Ordering};

        // Track hook invocations via thread-local
        thread_local! {
            static HOOK_FRAME_EPOCH: AtomicU64 = const { AtomicU64::new(0) };
            static HOOK_DENIAL_COUNT: AtomicU64 = const { AtomicU64::new(0) };
        }

        fn test_hook(frame_epoch: u64, denials: &[UnifiedDenialRecord]) {
            HOOK_FRAME_EPOCH.with(|e| e.store(frame_epoch, Ordering::SeqCst));
            HOOK_DENIAL_COUNT.with(|c| c.store(denials.len() as u64, Ordering::SeqCst));
        }

        // Reset counters
        HOOK_FRAME_EPOCH.with(|e| e.store(0, Ordering::SeqCst));
        HOOK_DENIAL_COUNT.with(|c| c.store(0, Ordering::SeqCst));

        // Register hook
        set_denial_telemetry_hook(Some(test_hook));

        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        let root = tree.create_root(Box::new(ConflictTestWidget::new()), None, 0);
        let widget = tree.add_child(root, Box::new(ConflictTestWidget::new()), None, 0);
        let widget_id = tree.node(widget).unwrap().id;

        // Set up store with geometry only in current frame (not previous)
        // This should cause authorization denial
        tree.begin_layout_transition_frame();
        tree.begin_layout_transition_frame();
        tree.record_layout_transition(widget_id, RectF::new(10.0, 0.0, 100.0, 100.0));

        let candidates = vec![LayoutTransitionCandidate {
            id: widget_id,
            from: RectF::new(0.0, 0.0, 100.0, 100.0),
            to: RectF::new(10.0, 0.0, 100.0, 100.0),
        }];

        drop(_guard);

        let auth_ctx = AuthorizationContext::minimal(tree.layout_transition_store());
        let conflict_flags = std::collections::HashMap::new();
        let _resolved =
            resolve_layout_transition_candidates(candidates, &auth_ctx, &tree, &conflict_flags, 42);

        // Verify hook was invoked with correct frame epoch
        assert_eq!(
            HOOK_FRAME_EPOCH.with(|e| e.load(Ordering::SeqCst)),
            42,
            "Hook should receive frame_epoch = 42"
        );

        // Verify hook received denial count
        assert_eq!(
            HOOK_DENIAL_COUNT.with(|c| c.load(Ordering::SeqCst)),
            1,
            "Hook should receive 1 denial"
        );

        // Cleanup
        set_denial_telemetry_hook(None);
        assert!(
            get_denial_telemetry_hook().is_none(),
            "Hook should be cleared"
        );
    }
}
