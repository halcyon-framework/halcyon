// ============================================================================
// SPEC-HAL-141: Rect Mutation Authority
// SPEC-HAL-145: Layout Transitions & Reflow Authority Enforcement
// ============================================================================
//
// This module enforces the rect mutation governance model from SPEC-HAL-141
// and the temporal authority model from SPEC-HAL-145.
//
// RELEASE-GRADE ENFORCEMENT: All checks run in both debug AND release builds.
// Authority tracking exists unconditionally. Violations panic in all builds.
//
// Authority domains (per SPEC-HAL-141 Section C + SPEC-HAL-145):
// 1. LayoutAuthority - default owner, assigned during layout resolution
//    - ONLY authority that may call set_rect()
//    - Owns spatial authority (who sets rects)
// 2. TransitionAuthority - interpolates between layout states
//    - Owns temporal authority (who interpolates over time)
//    - May NOT assign new rects (set_rect forbidden)
//    - May ONLY interpolate between two layout-authorized rects
// 3. AnimationAuthority - visual projection only (no direct rect mutation)
// 4. ScrollAuthority - viewport transforms only (no rect mutation)
//
// Per SPEC-HAL-141:
// - INV-RECT-LAYOUT-ONLY: Only layout engine may assign rects during layout
// - INV-RECT-AUTH-ENFORCED: Unauthorized mutation MUST panic (ALL BUILDS)
// - INV-RECT-O(1): Authority checks must be constant-time
//
// Per SPEC-HAL-145:
// - INV-TRANSITION-NO-RECT-ASSIGN: Transition authority MUST NOT call set_rect
// - INV-TRANSITION-INTERPOLATE-ONLY: Transition may only lerp between layout snapshots
// - INV-AUTH-MUTUAL-EXCLUSIVE: Layout and Transition authorities are mutually exclusive
// - INV-PAINT-NO-RECT-MUTATE: Paint phase may not mutate rects under any authority
//
// ============================================================================
// WIDGET GEOMETRY INVARIANTS (SPEC-HAL-141 Section D - Negative Constraints)
// ============================================================================
//
// These invariants define what widgets MUST NOT do. Violations are structural
// defects that cannot be "fixed later" — they indicate incorrect architecture.
//
// INV-WIDGET-NO-SHADOW-RECT:
//   Widgets MUST NOT store RectF fields representing widget/paint/viewport geometry.
//   WidgetNode is the sole owner of rect, absolute_xy, absolute_paint_rect, viewport.
//   Widget fields named `layout_bounds` (Size) are permitted for hit-testing only.
//
// INV-WIDGET-NO-GEOMETRY-DUPLICATION:
//   Widgets MUST NOT duplicate WidgetNode geometry in any form (Cell, RefCell, plain).
//   If geometry is needed, query WidgetNode through the provided context.
//
// INV-WIDGET-LAYOUT-BOUNDS-WRITE-PHASE:
//   Widget `layout_bounds` fields may ONLY be written during layout phase.
//   Event handlers and paint may read but never write.
//
// INV-WIDGET-VISUAL-PREFIX:
//   Any widget Cell<f32> or Cell<bool> storing visual/animation state that is
//   mutated during paint MUST be prefixed with `visual_`, `anim_`, or `blink_`.
//   These fields require AllowPaintMutationGuard to mutate.
//
// Enforcement:
//   - cargo check: no Cell<RectF|Point|Size> in src/widgets
//   - grep: no *_rect, *_viewport, *_origin fields without visual_ prefix
//   - Code review: any new geometry field requires SPEC approval
//
// ============================================================================

use std::cell::Cell;

/// Authority domain types per SPEC-HAL-141 Section C + SPEC-HAL-145.
///
/// At any moment, a rect may be owned by exactly one authority.
/// All rects default to LayoutAuthority unless explicitly transferred.
///
/// SPEC-HAL-145: Layout and Transition are mutually exclusive.
/// Only one may be active at a time (INV-AUTH-MUTUAL-EXCLUSIVE).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RectAuthority {
    /// Layout Authority: default and primary owner of rects.
    /// Only the layout engine may assign rects during layout resolution.
    /// SPEC-HAL-145: Owns SPATIAL authority (who sets rects).
    Layout,
    /// Transition Authority: owns geometry between two layout states.
    /// Interpolates only between layout-authorized rects.
    /// SPEC-HAL-145: Owns TEMPORAL authority (who interpolates over time).
    /// CANNOT call set_rect - interpolation only (INV-TRANSITION-NO-RECT-ASSIGN).
    Transition,
    /// No authority active - rect mutations are forbidden.
    None,
}

impl Default for RectAuthority {
    fn default() -> Self {
        RectAuthority::None
    }
}

// ============================================================================
// Thread-local authority tracking (ALL BUILDS - release-grade enforcement)
// ============================================================================

thread_local! {
    /// Current active rect mutation authority.
    /// None means no authority is active and rect mutations are forbidden.
    ///
    /// SPEC-HAL-141/145: This state exists in ALL builds for release-grade enforcement.
    static CURRENT_AUTHORITY: Cell<RectAuthority> = const { Cell::new(RectAuthority::None) };
}

/// Enter layout authority scope.
/// Called at the start of layout phase.
///
/// SPEC-HAL-141 INV-RECT-LAYOUT-EXCLUSIVE:
/// While layout owns a rect, no other system may mutate it.
///
/// # Panics
/// Panics if another authority is already active (INV-AUTH-MUTUAL-EXCLUSIVE).
#[inline]
pub fn enter_layout_authority() {
    CURRENT_AUTHORITY.with(|a| {
        let current = a.get();
        assert!(
            current == RectAuthority::None,
            "SPEC-HAL-141 VIOLATION: Cannot enter layout authority while another authority is active: {:?}",
            current
        );
        a.set(RectAuthority::Layout);
    });
}

/// Exit layout authority scope.
/// Called at the end of layout phase.
///
/// # Panics
/// Panics if not currently in layout authority.
#[inline]
pub fn exit_layout_authority() {
    CURRENT_AUTHORITY.with(|a| {
        let current = a.get();
        assert!(
            current == RectAuthority::Layout,
            "SPEC-HAL-141 VIOLATION: Cannot exit layout authority when not in layout: {:?}",
            current
        );
        a.set(RectAuthority::None);
    });
}

/// Check if layout authority is currently active.
#[inline]
pub fn is_layout_authority_active() -> bool {
    CURRENT_AUTHORITY.with(|a| a.get() == RectAuthority::Layout)
}

// ============================================================================
// SPEC-HAL-145: Transition Authority Functions
// ============================================================================

/// Enter transition authority scope.
/// Called at the start of transition interpolation phase.
///
/// SPEC-HAL-145 INV-AUTH-MUTUAL-EXCLUSIVE:
/// Cannot enter transition authority while layout authority is active.
///
/// SPEC-HAL-145 INV-TRANSITION-INTERPOLATE-ONLY:
/// Transition authority may only interpolate between layout snapshots.
/// Calling set_rect under transition authority will panic.
///
/// # Panics
/// Panics if another authority is already active.
#[inline]
pub fn enter_transition_authority() {
    CURRENT_AUTHORITY.with(|a| {
        let current = a.get();
        assert!(
            current == RectAuthority::None,
            "SPEC-HAL-145 VIOLATION: Cannot enter transition authority while another authority is active: {:?}.\n\
             INV-AUTH-MUTUAL-EXCLUSIVE: Layout and Transition are mutually exclusive.",
            current
        );
        a.set(RectAuthority::Transition);
    });
}

/// Exit transition authority scope.
/// Called at the end of transition interpolation phase.
///
/// # Panics
/// Panics if not currently in transition authority.
#[inline]
pub fn exit_transition_authority() {
    CURRENT_AUTHORITY.with(|a| {
        let current = a.get();
        assert!(
            current == RectAuthority::Transition,
            "SPEC-HAL-145 VIOLATION: Cannot exit transition authority when not in transition: {:?}",
            current
        );
        a.set(RectAuthority::None);
    });
}

/// Check if transition authority is currently active.
#[inline]
pub fn is_transition_authority_active() -> bool {
    CURRENT_AUTHORITY.with(|a| a.get() == RectAuthority::Transition)
}

/// Get current authority.
#[inline]
pub fn current_authority() -> RectAuthority {
    CURRENT_AUTHORITY.with(|a| a.get())
}

/// Assert that rect mutation (set_rect) is currently authorized.
///
/// SPEC-HAL-141 INV-RECT-AUTH-ENFORCED:
/// Unauthorized rect mutation MUST panic in ALL BUILDS.
///
/// SPEC-HAL-145 INV-TRANSITION-NO-RECT-ASSIGN:
/// Transition authority CANNOT call set_rect. Only Layout authority may assign rects.
///
/// This function is called by set_rect and restore_widget to enforce
/// that all rect assignments occur within Layout authority scope.
///
/// # Panics
/// Panics if current authority is not Layout.
#[inline]
pub fn assert_rect_mutation_authorized(context: &str) {
    CURRENT_AUTHORITY.with(|a| {
        let authority = a.get();
        assert!(
            authority == RectAuthority::Layout,
            "SPEC-HAL-141/145 VIOLATION: Unauthorized rect mutation attempted.\n\
             Context: {}\n\
             Current authority: {:?}\n\
             Required: Layout authority ONLY\n\
             \n\
             Rect mutations (set_rect) are ONLY permitted during:\n\
             - Layout phase (via LayoutContext methods)\n\
             \n\
             SPEC-HAL-145 INV-TRANSITION-NO-RECT-ASSIGN:\n\
             Transition authority may NOT assign rects, only interpolate.\n\
             \n\
             Ensure this rect mutation is happening within the layout phase.",
            context,
            authority
        );
    });
}

/// Assert that rect interpolation is currently authorized.
///
/// SPEC-HAL-145 INV-TRANSITION-INTERPOLATE-ONLY:
/// Only Transition authority may perform rect interpolation.
///
/// This function is called by transition interpolation code to verify
/// that interpolation occurs within Transition authority scope.
///
/// # Panics
/// Panics if current authority is not Transition.
#[inline]
pub fn assert_rect_interpolation_authorized(context: &str) {
    CURRENT_AUTHORITY.with(|a| {
        let authority = a.get();
        assert!(
            authority == RectAuthority::Transition,
            "SPEC-HAL-145 VIOLATION: Unauthorized rect interpolation attempted.\n\
             Context: {}\n\
             Current authority: {:?}\n\
             Required: Transition authority\n\
             \n\
             Rect interpolation is ONLY permitted during:\n\
             - Transition phase (via TransitionAuthorityGuard)\n\
             \n\
             Ensure this interpolation is happening within the transition phase.",
            context,
            authority
        );
    });
}

// ============================================================================
// RAII guard for layout authority scope
// ============================================================================

/// RAII guard that enters layout authority on creation and exits on drop.
///
/// Usage:
/// ```ignore
/// let _guard = LayoutAuthorityGuard::enter();
/// // ... layout code that mutates rects ...
/// // guard dropped, authority exits
/// ```
///
/// # Panics
/// - `enter()` panics if another authority is active
/// - `drop()` panics if authority state is corrupted (not in Layout)
pub struct LayoutAuthorityGuard {
    /// Marker to prevent Send/Sync (authority is thread-local)
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl LayoutAuthorityGuard {
    /// Enter layout authority scope.
    ///
    /// # Panics
    /// Panics if another authority is already active.
    #[inline]
    pub fn enter() -> Self {
        enter_layout_authority();
        Self {
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for LayoutAuthorityGuard {
    #[inline]
    fn drop(&mut self) {
        exit_layout_authority();
    }
}

// ============================================================================
// SPEC-HAL-145: RAII guard for transition authority scope
// ============================================================================

/// RAII guard that enters transition authority on creation and exits on drop.
///
/// SPEC-HAL-145: Transition authority owns temporal interpolation between layouts.
/// Under this authority:
/// - Rect interpolation (lerp_rect) is permitted
/// - Rect mutation (set_rect) is FORBIDDEN (INV-TRANSITION-NO-RECT-ASSIGN)
///
/// Usage:
/// ```ignore
/// let _guard = TransitionAuthorityGuard::enter();
/// // ... transition interpolation code ...
/// let visual_rect = lerp_rect(from_rect, to_rect, progress);
/// // guard dropped, authority exits
/// ```
///
/// # Panics
/// - `enter()` panics if another authority is active
/// - `drop()` panics if authority state is corrupted (not in Transition)
pub struct TransitionAuthorityGuard {
    /// Marker to prevent Send/Sync (authority is thread-local)
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl TransitionAuthorityGuard {
    /// Enter transition authority scope.
    ///
    /// SPEC-HAL-145 INV-AUTH-MUTUAL-EXCLUSIVE:
    /// Will panic if layout authority is active.
    ///
    /// # Panics
    /// Panics if another authority is already active.
    #[inline]
    pub fn enter() -> Self {
        enter_transition_authority();
        Self {
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for TransitionAuthorityGuard {
    #[inline]
    fn drop(&mut self) {
        exit_transition_authority();
    }
}

// ============================================================================
// SPEC-HAL-146: Update-phase authority (debug-only)
// ============================================================================
//
// Update phase runs BEFORE layout and is the only legal time to mutate
// widget-internal visual state like scroll_offset_visual.
//
// This is debug-only enforcement - it verifies temporal ownership without
// affecting release performance.

thread_local! {
    /// SPEC-HAL-146: Track whether Update phase is active.
    /// Debug-only enforcement for visual state mutations.
    static UPDATE_PHASE_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// Enter Update-phase authority scope.
/// Called at the start of widget update traversal.
///
/// SPEC-HAL-146: Update phase is the only legal time to mutate
/// widget-internal visual state (scroll_offset_visual, etc).
///
/// # Panics (debug only)
/// Panics if Update phase is already active (re-entrant call).
#[inline]
pub fn enter_update_authority() {
    #[cfg(debug_assertions)]
    UPDATE_PHASE_ACTIVE.with(|a| {
        assert!(
            !a.get(),
            "SPEC-HAL-146 VIOLATION: Cannot enter Update phase while already in Update phase"
        );
        a.set(true);
    });
}

/// Exit Update-phase authority scope.
/// Called at the end of widget update traversal.
///
/// # Panics (debug only)
/// Panics if not currently in Update phase.
#[inline]
pub fn exit_update_authority() {
    #[cfg(debug_assertions)]
    UPDATE_PHASE_ACTIVE.with(|a| {
        assert!(
            a.get(),
            "SPEC-HAL-146 VIOLATION: Cannot exit Update phase when not in Update phase"
        );
        a.set(false);
    });
}

/// Check if Update-phase authority is currently active.
#[inline]
pub fn is_update_authority_active() -> bool {
    #[cfg(debug_assertions)]
    {
        UPDATE_PHASE_ACTIVE.with(|a| a.get())
    }
    #[cfg(not(debug_assertions))]
    {
        true // Always "authorized" in release (no tracking overhead)
    }
}

/// Assert that Update-phase mutation is currently authorized.
///
/// SPEC-HAL-146: Widget-internal visual state mutations (like scroll_offset_visual)
/// are ONLY permitted during the Update phase, before layout.
///
/// # Panics (debug only)
/// Panics if current phase is not Update.
#[inline]
pub fn assert_update_mutation_authorized(context: &str) {
    #[cfg(debug_assertions)]
    UPDATE_PHASE_ACTIVE.with(|a| {
        assert!(
            a.get(),
            "SPEC-HAL-146 VIOLATION: Unauthorized Update-phase mutation attempted.\n\
             Context: {}\n\
             Current phase: NOT Update\n\
             \n\
             Widget-internal visual state mutations are ONLY permitted during:\n\
             - Update phase (via UpdateAuthorityGuard)\n\
             \n\
             Ensure this mutation is happening within the widget update traversal.",
            context
        );
    });
    #[cfg(not(debug_assertions))]
    {
        let _ = context; // Suppress unused warning in release
    }
}

/// RAII guard that enters Update-phase authority on creation and exits on drop.
///
/// SPEC-HAL-146: Update phase is the only legal time to mutate
/// widget-internal visual state like scroll_offset_visual.
///
/// Usage:
/// ```ignore
/// let _guard = UpdateAuthorityGuard::enter();
/// // ... widget update code that mutates visual state ...
/// // guard dropped, authority exits
/// ```
///
/// # Panics (debug only)
/// - `enter()` panics if already in Update phase
/// - `drop()` panics if not in Update phase
pub struct UpdateAuthorityGuard {
    /// Marker to prevent Send/Sync (authority is thread-local)
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl UpdateAuthorityGuard {
    /// Enter Update-phase authority scope.
    ///
    /// # Panics (debug only)
    /// Panics if already in Update phase.
    #[inline]
    pub fn enter() -> Self {
        enter_update_authority();
        Self {
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for UpdateAuthorityGuard {
    #[inline]
    fn drop(&mut self) {
        exit_update_authority();
    }
}

// ============================================================================
// Paint-phase activity (all builds), SPEC-HAL-146 mutation checks (debug-only)
// ============================================================================
//
// Paint phase runs AFTER layout and transition. Visual state mutations during
// paint are DENY BY DEFAULT. Only explicitly whitelisted call sites may mutate
// by opening an AllowPaintMutationGuard scope.

thread_local! {
    /// Actual traversal activity for HAL359 exclusion, independent of mutation permission.
    /// Depth preserves release nesting; debug retains its existing re-entry rejection.
    static PAINT_PHASE_DEPTH: Cell<usize> = const { Cell::new(0) };

    /// SPEC-HAL-146: Paint mutation gate - DENY BY DEFAULT.
    /// Only AllowPaintMutationGuard can temporarily open mutation.
    static PAINT_MUTATION_ALLOWED: Cell<bool> = const { Cell::new(false) };
}

/// Enter Paint-phase authority scope.
/// Called at the start of paint traversal.
///
/// SPEC-HAL-146: Paint phase is DENY BY DEFAULT for mutations.
/// Call sites must explicitly open AllowPaintMutationGuard to mutate.
///
/// # Panics (debug only)
/// Panics if Paint phase is already active (re-entrant call).
#[inline]
pub fn enter_paint_authority() {
    PAINT_PHASE_DEPTH.with(|a| {
        #[cfg(debug_assertions)]
        assert!(
            a.get() == 0,
            "SPEC-HAL-146 VIOLATION: Cannot enter Paint phase while already in Paint phase"
        );
        a.set(a.get().checked_add(1).expect("Paint phase depth overflow"));
    });
    // Default deny: mutation not allowed until explicitly opened.
    #[cfg(debug_assertions)]
    PAINT_MUTATION_ALLOWED.with(|a| a.set(false));
}

/// Exit Paint-phase authority scope.
/// Called at the end of paint traversal.
///
/// # Panics
/// Panics if not currently in Paint phase.
#[inline]
pub fn exit_paint_authority() {
    PAINT_PHASE_DEPTH.with(|a| {
        a.set(
            a.get()
                .checked_sub(1)
                .expect("SPEC-HAL-146 VIOLATION: Cannot exit Paint phase when not in Paint phase"),
        );
    });
    // Ensure mutation gate is closed.
    #[cfg(debug_assertions)]
    PAINT_MUTATION_ALLOWED.with(|a| a.set(false));
}

/// Actual Paint traversal activity in every build, for mutation exclusion.
#[inline]
pub(crate) fn is_paint_phase_active() -> bool {
    PAINT_PHASE_DEPTH.with(|a| a.get() != 0)
}

/// Legacy Paint authority check: tracks activity in debug and permits release.
/// Mutation exclusion must use `is_paint_phase_active`, not this relaxed check.
#[inline]
pub fn is_paint_authority_active() -> bool {
    #[cfg(debug_assertions)]
    {
        is_paint_phase_active()
    }
    #[cfg(not(debug_assertions))]
    {
        true // Legacy release permission, not traversal activity.
    }
}

/// Assert that Paint-phase mutation is currently authorized.
///
/// SPEC-HAL-146: Paint-time visual state mutations are DENY BY DEFAULT.
/// This assert checks BOTH:
/// 1. Paint phase is active
/// 2. AllowPaintMutationGuard is currently open
///
/// # Panics (debug only)
/// Panics if not in Paint phase OR if mutation is not explicitly allowed.
#[inline]
pub fn assert_paint_mutation_authorized(context: &str) {
    #[cfg(debug_assertions)]
    {
        let phase_active = is_paint_phase_active();
        let mutation_allowed = PAINT_MUTATION_ALLOWED.with(|a| a.get());
        assert!(
            phase_active && mutation_allowed,
            "SPEC-HAL-146 VIOLATION: Unauthorized Paint-phase mutation attempted.\n\
             Context: {}\n\
             Paint phase active: {}\n\
             Mutation allowed: {}\n\
             \n\
             Paint-time mutations are DENY BY DEFAULT.\n\
             You must wrap mutations in AllowPaintMutationGuard::enter() scope.\n\
             \n\
             Example:\n\
             let _allow = AllowPaintMutationGuard::enter();\n\
             self.some_cell.set(value);",
            context,
            phase_active,
            mutation_allowed
        );
    }
    #[cfg(not(debug_assertions))]
    {
        let _ = context; // Suppress unused warning in release
    }
}

/// RAII guard that enters Paint-phase authority on creation and exits on drop.
///
/// SPEC-HAL-146: Paint phase permits visual state mutations (animation reads,
/// caret blink) but NOT geometry mutations.
///
/// Usage:
/// ```ignore
/// let _guard = PaintAuthorityGuard::enter();
/// // ... paint code that may read animations / update caret blink ...
/// // guard dropped, authority exits
/// ```
///
/// # Panics
/// - In debug builds, `enter()` panics if already in Paint phase.
/// - In every build, `drop()` panics if the scope is unbalanced.
pub struct PaintAuthorityGuard {
    /// Marker to prevent Send/Sync (authority is thread-local)
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl PaintAuthorityGuard {
    /// Enter Paint-phase authority scope.
    ///
    /// # Panics (debug only)
    /// Panics if already in Paint phase.
    #[inline]
    pub fn enter() -> Self {
        enter_paint_authority();
        Self {
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for PaintAuthorityGuard {
    #[inline]
    fn drop(&mut self) {
        exit_paint_authority();
    }
}

// ============================================================================
// SPEC-HAL-146: Allow-paint-mutation guard (debug-only)
// ============================================================================

/// RAII guard that temporarily allows paint-phase mutations.
///
/// SPEC-HAL-146: Paint mutations are DENY BY DEFAULT. This guard opens a
/// local "allow window" for whitelisted call sites that must mutate during paint.
///
/// Usage:
/// ```ignore
/// // Inside paint(), before mutating:
/// let _allow = AllowPaintMutationGuard::enter();
/// self.some_cell.set(value);  // Now allowed
/// // guard dropped, mutation disallowed again
/// ```
///
/// # Panics (debug only)
/// - `enter()` panics if not in Paint phase
/// - `enter()` panics if already allowed (no nesting)
pub struct AllowPaintMutationGuard {
    /// Marker to prevent Send/Sync (authority is thread-local)
    _not_send_sync: std::marker::PhantomData<*const ()>,
}

impl AllowPaintMutationGuard {
    /// Open a paint-mutation allow window.
    ///
    /// # Panics (debug only)
    /// - Panics if not currently in Paint phase
    /// - Panics if mutation is already allowed (no nesting)
    #[inline]
    pub fn enter() -> Self {
        #[cfg(debug_assertions)]
        {
            assert!(
                is_paint_phase_active(),
                "SPEC-HAL-146 VIOLATION: AllowPaintMutationGuard requires Paint phase to be active"
            );
            PAINT_MUTATION_ALLOWED.with(|a| {
                assert!(
                    !a.get(),
                    "SPEC-HAL-146 VIOLATION: AllowPaintMutationGuard cannot nest (already allowed)"
                );
                a.set(true);
            });
        }
        Self {
            _not_send_sync: std::marker::PhantomData,
        }
    }
}

impl Drop for AllowPaintMutationGuard {
    #[inline]
    fn drop(&mut self) {
        #[cfg(debug_assertions)]
        PAINT_MUTATION_ALLOWED.with(|a| a.set(false));
    }
}

// ============================================================================
// Test utilities
// ============================================================================

/// Reset authority state to None. For test cleanup only.
///
/// # Safety
/// This bypasses normal authority flow and should only be used in tests
/// to ensure clean state between test runs.
#[cfg(test)]
pub fn reset_authority_state_for_test() {
    CURRENT_AUTHORITY.with(|a| a.set(RectAuthority::None));
}

// ============================================================================
// Tests - run in ALL builds (not just debug)
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper to reset thread-local state before each test.
    /// Tests run in parallel and may share threads, so we need to ensure clean state.
    fn reset_authority_state() {
        reset_authority_state_for_test();
    }

    #[test]
    fn test_layout_authority_lifecycle() {
        reset_authority_state();

        // Initially no authority
        assert!(!is_layout_authority_active());
        assert_eq!(current_authority(), RectAuthority::None);

        // Enter layout authority
        let _guard = LayoutAuthorityGuard::enter();
        assert!(is_layout_authority_active());
        assert_eq!(current_authority(), RectAuthority::Layout);

        // Guard dropped, authority exits
        drop(_guard);
        assert!(!is_layout_authority_active());
        assert_eq!(current_authority(), RectAuthority::None);
    }

    #[test]
    fn test_authorized_mutation_passes() {
        reset_authority_state();

        let _guard = LayoutAuthorityGuard::enter();
        // Should not panic
        assert_rect_mutation_authorized("test context");
    }

    #[test]
    fn test_authority_state_transitions() {
        reset_authority_state();
        assert_eq!(current_authority(), RectAuthority::None);

        // Enter layout authority
        enter_layout_authority();
        assert_eq!(current_authority(), RectAuthority::Layout);

        // Exit layout authority
        exit_layout_authority();
        assert_eq!(current_authority(), RectAuthority::None);
    }

    // ========================================================================
    // SPEC-HAL-145: Transition Authority Tests
    // ========================================================================

    #[test]
    fn test_transition_authority_lifecycle() {
        reset_authority_state();

        // Initially no authority
        assert!(!is_transition_authority_active());
        assert!(!is_layout_authority_active());

        // Enter transition authority
        let _guard = TransitionAuthorityGuard::enter();
        assert!(is_transition_authority_active());
        assert!(!is_layout_authority_active());
        assert_eq!(current_authority(), RectAuthority::Transition);

        // Guard dropped, authority exits
        drop(_guard);
        assert!(!is_transition_authority_active());
        assert_eq!(current_authority(), RectAuthority::None);
    }

    #[test]
    fn test_transition_authority_state_transitions() {
        reset_authority_state();
        assert_eq!(current_authority(), RectAuthority::None);

        // Enter transition authority
        enter_transition_authority();
        assert_eq!(current_authority(), RectAuthority::Transition);

        // Exit transition authority
        exit_transition_authority();
        assert_eq!(current_authority(), RectAuthority::None);
    }

    #[test]
    fn test_interpolation_authorized_under_transition() {
        reset_authority_state();

        let _guard = TransitionAuthorityGuard::enter();
        // Should not panic - interpolation is allowed under transition authority
        assert_rect_interpolation_authorized("test context");
    }

    /// SPEC-HAL-145 INV-AUTH-MUTUAL-EXCLUSIVE:
    /// Layout and Transition authorities are mutually exclusive.
    /// This test verifies that entering one while the other is active
    /// results in expected behavior (assertions would fire in debug).
    #[test]
    fn test_authorities_mutually_exclusive_sequential() {
        reset_authority_state();

        // Verify layout then transition works when properly sequenced
        {
            let _layout = LayoutAuthorityGuard::enter();
            assert!(is_layout_authority_active());
        }
        // Layout guard dropped

        {
            let _transition = TransitionAuthorityGuard::enter();
            assert!(is_transition_authority_active());
        }
        // Transition guard dropped

        // Verify transition then layout works when properly sequenced
        {
            let _transition = TransitionAuthorityGuard::enter();
            assert!(is_transition_authority_active());
        }

        {
            let _layout = LayoutAuthorityGuard::enter();
            assert!(is_layout_authority_active());
        }
    }

    #[test]
    fn test_layout_authority_required_for_rect_mutation() {
        reset_authority_state();

        // Layout authority allows rect mutation
        let _guard = LayoutAuthorityGuard::enter();
        assert_rect_mutation_authorized("test context");
        // Should not panic
    }

    /// Test that Layout -> Transition -> Paint order is enforceable.
    /// This simulates the frame sequence from SPEC-HAL-145.
    #[test]
    fn test_layout_transition_paint_order() {
        reset_authority_state();

        // Phase 1: Layout (rect assignment)
        {
            let _layout = LayoutAuthorityGuard::enter();
            assert!(is_layout_authority_active());
            assert_rect_mutation_authorized("layout phase");
        }

        // Phase 2: Transition (interpolation)
        {
            let _transition = TransitionAuthorityGuard::enter();
            assert!(is_transition_authority_active());
            assert_rect_interpolation_authorized("transition phase");
        }

        // Phase 3: Paint (read-only, no authority required for painting)
        assert_eq!(current_authority(), RectAuthority::None);
        // Paint just reads rects, doesn't mutate or interpolate them
    }

    // ========================================================================
    // Release-grade enforcement tests (these verify panic behavior)
    // ========================================================================

    #[test]
    #[should_panic(expected = "SPEC-HAL-141/145 VIOLATION")]
    fn test_unauthorized_rect_mutation_panics() {
        reset_authority_state();
        // No authority active - this should panic
        assert_rect_mutation_authorized("unauthorized mutation test");
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-145 VIOLATION")]
    fn test_unauthorized_interpolation_panics() {
        reset_authority_state();
        // No authority active - this should panic
        assert_rect_interpolation_authorized("unauthorized interpolation test");
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-141/145 VIOLATION")]
    fn test_rect_mutation_under_transition_panics() {
        reset_authority_state();
        let _guard = TransitionAuthorityGuard::enter();
        // Transition authority does NOT allow rect mutation
        assert_rect_mutation_authorized("mutation under transition test");
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-145 VIOLATION")]
    fn test_interpolation_under_layout_panics() {
        reset_authority_state();
        let _guard = LayoutAuthorityGuard::enter();
        // Layout authority does NOT allow interpolation
        assert_rect_interpolation_authorized("interpolation under layout test");
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-141 VIOLATION")]
    fn test_double_layout_entry_panics() {
        reset_authority_state();
        enter_layout_authority();
        // Already in layout - this should panic
        enter_layout_authority();
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-145 VIOLATION")]
    fn test_double_transition_entry_panics() {
        reset_authority_state();
        enter_transition_authority();
        // Already in transition - this should panic
        enter_transition_authority();
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-145 VIOLATION")]
    fn test_transition_during_layout_panics() {
        reset_authority_state();
        enter_layout_authority();
        // Layout is active - entering transition should panic
        enter_transition_authority();
    }

    #[test]
    #[should_panic(expected = "SPEC-HAL-141 VIOLATION")]
    fn test_layout_during_transition_panics() {
        reset_authority_state();
        enter_transition_authority();
        // Transition is active - entering layout should panic
        enter_layout_authority();
    }
}
