use halcyon::model::navigation_host::{
    GuardDecision, NavigationAuthorizationResult, NavigationDecision, NavigationEntry,
    NavigationGuard, NavigationGuardContextGeneration, NavigationHost, NavigationIntent,
    NavigationNodeId, NavigationOutcome, NavigationPreparationError, NavigationResolution,
    NavigationState,
};
use std::any::Any;
use std::cell::Cell;
use std::rc::Rc;

fn nid(id: u32) -> NavigationNodeId {
    NavigationNodeId(id)
}

// --- Guard implementations for tests ---

/// Denies NavigateUp, allows everything else.
struct DenyBackGuard;

impl NavigationGuard for DenyBackGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        match intent {
            NavigationIntent::NavigateUp => GuardDecision::Deny,
            _ => GuardDecision::Allow,
        }
    }
}

/// Redirects NavigateTo(source) to NavigateTo(target), allows everything else.
struct RedirectGuard {
    source: NavigationNodeId,
    target: NavigationNodeId,
}

impl NavigationGuard for RedirectGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        match intent {
            NavigationIntent::NavigateTo(entry) if entry.node_id() == self.source => {
                GuardDecision::ReplaceWith(NavigationIntent::NavigateTo(self.target.into()))
            }
            _ => GuardDecision::Allow,
        }
    }
}

/// Always returns ReplaceWith the same intent (infinite loop trigger).
struct InfiniteReplaceGuard;

impl NavigationGuard for InfiniteReplaceGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        _intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        GuardDecision::ReplaceWith(NavigationIntent::NavigateTo(nid(99).into()))
    }
}

/// Denies all navigation.
struct DenyAllGuard;

impl NavigationGuard for DenyAllGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        _intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        GuardDecision::Deny
    }
}

// --- Tests ---

// 1. Deny blocks Back without state change (INV-NG-231-04)
#[test]
fn deny_blocks_back_without_state_change() {
    let mut host = NavigationHost::new(nid(1));
    host.apply(NavigationIntent::NavigateTo(nid(2).into()));
    host.apply(NavigationIntent::NavigateTo(nid(3).into()));

    // Snapshot state before denied Back
    let path_before = host.path().to_vec();
    let current_before = host.current();

    host.set_guard(Box::new(DenyBackGuard));
    host.apply(NavigationIntent::NavigateUp);

    // State unchanged (INV-NG-231-04)
    assert_eq!(host.current(), current_before);
    assert_eq!(host.path(), path_before.as_slice());
}

// 2. ReplaceWith redirects; original intent never executed (INV-NG-231-06)
#[test]
fn replacewith_redirects_and_original_not_executed() {
    let mut host = NavigationHost::new(nid(1));

    // Guard: NavigateTo(5) → NavigateTo(10)
    host.set_guard(Box::new(RedirectGuard {
        source: nid(5),
        target: nid(10),
    }));

    host.apply(NavigationIntent::NavigateTo(nid(5).into()));

    // Final state is the replacement target, not the original
    assert_eq!(host.current(), nid(10));
    assert!(host.path().contains(&nid(10)));
    // Original target was never executed (no nid(5) in path)
    assert!(!host.path().contains(&nid(5)));
}

// 3. No guard installed: behavior unchanged (INV-NG-231-08)
#[test]
fn no_guard_installed_behavior_unchanged() {
    let mut host = NavigationHost::new(nid(1));
    host.apply(NavigationIntent::NavigateTo(nid(2).into()));
    host.apply(NavigationIntent::NavigateTo(nid(3).into()));

    assert_eq!(host.current(), nid(3));
    assert_eq!(host.path(), &[nid(1), nid(2), nid(3)]);

    // Back works normally
    host.apply(NavigationIntent::NavigateUp);
    assert_eq!(host.current(), nid(2));
    assert_eq!(host.path(), &[nid(1), nid(2)]);

    // NavigateTo works normally
    host.apply(NavigationIntent::NavigateTo(nid(4).into()));
    assert_eq!(host.current(), nid(4));
    assert_eq!(host.path(), &[nid(1), nid(2), nid(4)]);
}

// 4. ReplaceWith loop hits safety cap and fails safe (deny-equivalent)
#[test]
fn replacewith_loop_cap_fails_safe() {
    let mut host = NavigationHost::new(nid(1));
    host.apply(NavigationIntent::NavigateTo(nid(2).into()));

    let path_before = host.path().to_vec();
    let current_before = host.current();

    // Guard always returns ReplaceWith — triggers safety cap
    host.set_guard(Box::new(InfiniteReplaceGuard));
    host.apply(NavigationIntent::NavigateTo(nid(3).into()));

    // State unchanged — safety cap denied
    assert_eq!(host.current(), current_before);
    assert_eq!(host.path(), path_before.as_slice());
}

// 5. decide() is semantic-only: returns decisions without surface state (INV-NG-231-07)
#[test]
fn decide_is_semantic_only() {
    let mut host = NavigationHost::new(nid(1));

    // Deny: no state change
    host.set_guard(Box::new(DenyAllGuard));
    let decision = host.decide(NavigationIntent::NavigateTo(nid(2).into()));
    assert_eq!(decision, NavigationDecision::Deny);
    assert_eq!(host.current(), nid(1));
    assert_eq!(host.path(), &[nid(1)]);

    // Clear guard: Allow succeeds, state changes
    host.clear_guard();
    let decision = host.decide(NavigationIntent::NavigateTo(nid(2).into()));
    assert_eq!(decision, NavigationDecision::Allow);
    assert_eq!(host.current(), nid(2));
    assert_eq!(host.path(), &[nid(1), nid(2)]);

    // Back
    let decision = host.decide(NavigationIntent::NavigateUp);
    assert_eq!(decision, NavigationDecision::Allow);
    assert_eq!(host.current(), nid(1));
    assert_eq!(host.path(), &[nid(1)]);
}

struct CountingGuard {
    evaluations: Rc<Cell<usize>>,
}

impl NavigationGuard for CountingGuard {
    fn evaluate(
        &self,
        _requested_intent: &NavigationIntent,
        _evaluated_intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        self.evaluations.set(self.evaluations.get() + 1);
        GuardDecision::Allow
    }
}

#[test]
fn unavailable_and_noop_skip_guard_and_leave_generation_unchanged() {
    let evaluations = Rc::new(Cell::new(0));
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(CountingGuard {
        evaluations: Rc::clone(&evaluations),
    }));
    let generation = host.snapshot().unwrap().generation();

    assert_eq!(
        host.execute(NavigationIntent::NavigateUp, None, None),
        Ok(NavigationOutcome::Unavailable)
    );
    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(1).into()), None, None),
        Ok(NavigationOutcome::NoOp)
    );
    assert_eq!(evaluations.get(), 0);
    assert_eq!(host.snapshot().unwrap().generation(), generation);
    assert_eq!(host.path(), &[nid(1)]);
}

#[test]
fn history_traversal_moves_cursor_and_new_navigation_truncates_forward_branch() {
    let host = NavigationHost::new(nid(1));
    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(2).into()), None, None),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(3).into()), None, None),
        Ok(NavigationOutcome::Committed { redirected: false })
    );

    assert_eq!(
        host.execute(NavigationIntent::HistoryBack, None, None),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(host.current(), nid(2));
    let projection = host.control_projection();
    assert!(projection.can_history_back);
    assert!(projection.can_history_forward);

    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(4).into()), None, None),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(host.path(), &[nid(1), nid(2), nid(4)]);
    assert!(!host.control_projection().can_history_forward);
    assert_eq!(host.snapshot().unwrap().history_len(), 3);
}

#[test]
fn navigate_path_uses_complete_entry_identity_and_rejects_foreign_or_duplicate_paths() {
    let root = NavigationEntry::new(nid(1)).with_restoration_key("root");
    let account_a = NavigationEntry::new(nid(2)).with_parameter("account", "a");
    let account_b = NavigationEntry::new(nid(2)).with_parameter("account", "b");
    let host = NavigationHost::new(root.clone());

    assert_eq!(
        host.execute(
            NavigationIntent::NavigatePath(vec![root.clone(), account_a.clone()]),
            None,
            None,
        ),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(host.current_entry(), account_a);

    assert_eq!(
        host.execute(
            NavigationIntent::NavigatePath(vec![root.clone(), account_b.clone()]),
            None,
            None,
        ),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(host.current_entry(), account_b);

    assert_eq!(
        host.execute(
            NavigationIntent::NavigatePath(vec![NavigationEntry::new(nid(99))]),
            None,
            None,
        ),
        Ok(NavigationOutcome::Unavailable)
    );
    assert_eq!(
        host.execute(
            NavigationIntent::NavigatePath(vec![root.clone(), root]),
            None,
            None,
        ),
        Ok(NavigationOutcome::Unavailable)
    );
}

struct RequestedIntentGuard {
    requested_was_original: Rc<Cell<bool>>,
}

impl NavigationGuard for RequestedIntentGuard {
    fn evaluate(
        &self,
        requested_intent: &NavigationIntent,
        evaluated_intent: &NavigationIntent,
        _from: &NavigationState,
        _to: &NavigationState,
        _context: Option<&dyn Any>,
    ) -> GuardDecision {
        match evaluated_intent {
            NavigationIntent::NavigateTo(entry) if entry.node_id() == nid(5) => {
                GuardDecision::ReplaceWith(NavigationIntent::NavigateTo(nid(10).into()))
            }
            NavigationIntent::NavigateTo(entry) if entry.node_id() == nid(10) => {
                self.requested_was_original.set(matches!(
                    requested_intent,
                    NavigationIntent::NavigateTo(requested) if requested.node_id() == nid(5)
                ));
                GuardDecision::Allow
            }
            _ => GuardDecision::Deny,
        }
    }
}

#[test]
fn replacement_chain_preserves_requested_intent_and_commits_only_final_candidate() {
    let requested_was_original = Rc::new(Cell::new(false));
    let mut host = NavigationHost::new(nid(1));
    host.set_guard(Box::new(RequestedIntentGuard {
        requested_was_original: Rc::clone(&requested_was_original),
    }));

    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(5).into()), None, None),
        Ok(NavigationOutcome::Committed { redirected: true })
    );
    assert!(requested_was_original.get());
    assert_eq!(host.path(), &[nid(1), nid(10)]);
}

fn authorized_candidate(
    host: &NavigationHost,
    intent: NavigationIntent,
    guard_generation: Option<NavigationGuardContextGeneration>,
) -> (
    halcyon::model::navigation_host::NavigationMechanicsSnapshot,
    halcyon::model::navigation_host::NavigationGuardAuthorization,
) {
    let snapshot = host.snapshot().unwrap();
    let authorization = match host.authorize(&snapshot, intent, None, guard_generation) {
        NavigationAuthorizationResult::Authorized(authorization) => authorization,
        other => panic!("expected authorization, got {other:?}"),
    };
    (snapshot, authorization)
}

#[test]
fn prepared_commit_exclusively_leases_generation_and_drop_is_mutation_free() {
    let host = NavigationHost::new(nid(1));
    let (snapshot, authorization) =
        authorized_candidate(&host, NavigationIntent::NavigateTo(nid(2).into()), None);
    let prepared = host
        .prepare_commit(authorization, snapshot.generation(), None)
        .unwrap();

    let (_, competing_authorization) =
        authorized_candidate(&host, NavigationIntent::NavigateTo(nid(3).into()), None);
    assert_eq!(
        host.prepare_commit(competing_authorization, snapshot.generation(), None)
            .unwrap_err(),
        NavigationPreparationError::Busy
    );
    assert_eq!(host.path(), &[nid(1)]);
    assert_eq!(host.snapshot().unwrap().generation(), snapshot.generation());

    drop(prepared);
    let (replacement_snapshot, replacement_authorization) =
        authorized_candidate(&host, NavigationIntent::NavigateTo(nid(3).into()), None);
    let replacement = host
        .prepare_commit(
            replacement_authorization,
            replacement_snapshot.generation(),
            None,
        )
        .unwrap();
    assert_eq!(
        replacement.commit(),
        NavigationOutcome::Committed { redirected: false }
    );
    assert_eq!(host.path(), &[nid(1), nid(3)]);
    assert_eq!(
        host.snapshot().unwrap().generation().get(),
        snapshot.generation().get() + 1
    );
}

#[test]
fn stale_host_or_guard_context_revalidation_cannot_prepare() {
    let host = NavigationHost::new(nid(1));
    let guard_generation = NavigationGuardContextGeneration::new(7);
    let (snapshot, authorization) = authorized_candidate(
        &host,
        NavigationIntent::NavigateTo(nid(2).into()),
        Some(guard_generation),
    );
    assert_eq!(
        host.prepare_commit(
            authorization,
            snapshot.generation(),
            Some(NavigationGuardContextGeneration::new(8)),
        )
        .unwrap_err(),
        NavigationPreparationError::StaleGuardContext
    );
    assert_eq!(host.path(), &[nid(1)]);

    let (stale_snapshot, stale_authorization) =
        authorized_candidate(&host, NavigationIntent::NavigateTo(nid(2).into()), None);
    assert_eq!(
        host.execute(NavigationIntent::NavigateTo(nid(3).into()), None, None),
        Ok(NavigationOutcome::Committed { redirected: false })
    );
    assert_eq!(
        host.prepare_commit(stale_authorization, stale_snapshot.generation(), None)
            .unwrap_err(),
        NavigationPreparationError::StaleGeneration
    );
    assert_eq!(host.path(), &[nid(1), nid(3)]);
}

#[test]
fn candidate_from_another_host_is_retry_and_cannot_authorize_here() {
    let first = NavigationHost::new(nid(1));
    let second = NavigationHost::new(nid(10));
    let snapshot = first.snapshot().unwrap();
    let candidate = match first.resolve(&snapshot, NavigationIntent::NavigateTo(nid(2).into())) {
        NavigationResolution::Candidate(candidate) => candidate,
        other => panic!("expected candidate, got {other:?}"),
    };
    assert!(matches!(
        second.authorize_candidate(&snapshot, candidate, None, None),
        NavigationAuthorizationResult::Retry
    ));
    assert_eq!(second.path(), &[nid(10)]);
}
