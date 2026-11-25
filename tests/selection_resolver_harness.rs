use halcyon::model::selection_resolver::{
    CommitSource, CommittedValue, OptionEntry, OptionId, RejectionReason, ResolverDecision,
    ResolverHandle, ResolverInput, SelectionResolver,
};

/// A trivial deterministic resolver used as an oracle.
/// Rules:
/// - If CommitSource::Selection and highlighted_id exists → commit that id
/// - If CommitSource::InputConfirm and input_text matches exactly one option label → commit that id
/// - Otherwise → Reject
struct TestResolver;

impl TestResolver {
    fn handle() -> ResolverHandle {
        ResolverHandle::new(Self)
    }
}

impl SelectionResolver for TestResolver {
    fn resolve(&self, input: &ResolverInput) -> ResolverDecision {
        match input.commit_source {
            CommitSource::Selection => {
                if let Some(id) = input.highlighted_id {
                    ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                } else {
                    ResolverDecision::Reject(RejectionReason::NoMatch)
                }
            }
            CommitSource::InputConfirm => {
                let text = match input.input_text {
                    Some(t) => t,
                    None => return ResolverDecision::Reject(RejectionReason::EmptyInput),
                };

                let mut matches = input
                    .options
                    .iter()
                    .filter(|opt| opt.label == text)
                    .map(|opt| opt.id);

                match (matches.next(), matches.next()) {
                    (Some(id), None) => {
                        ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                    }
                    _ => ResolverDecision::Reject(RejectionReason::NoMatch),
                }
            }
        }
    }
}

#[test]
fn selection_commit_via_highlight() {
    let resolver = TestResolver::handle();

    let options = vec![
        (OptionId::from_raw(1), "One"),
        (OptionId::from_raw(2), "Two"),
    ];
    let entries: Vec<OptionEntry<'_>> = options
        .iter()
        .map(|(id, label)| OptionEntry { id: *id, label })
        .collect();

    let input = ResolverInput {
        input_text: None,
        highlighted_id: Some(OptionId::from_raw(2)),
        commit_source: CommitSource::Selection,
        options: &entries,
    };

    match resolver.resolve(&input) {
        ResolverDecision::Commit(CommittedValue::ExistingOption(id)) => {
            assert_eq!(id, OptionId::from_raw(2));
        }
        other => panic!("expected Commit(ExistingOption), got {:?}", other),
    }
}

#[test]
fn input_confirm_exact_match() {
    let resolver = TestResolver::handle();

    let options = vec![
        (OptionId::from_raw(1), "Apple"),
        (OptionId::from_raw(2), "Banana"),
    ];
    let entries: Vec<OptionEntry<'_>> = options
        .iter()
        .map(|(id, label)| OptionEntry { id: *id, label })
        .collect();

    let input = ResolverInput {
        input_text: Some("Banana"),
        highlighted_id: None,
        commit_source: CommitSource::InputConfirm,
        options: &entries,
    };

    match resolver.resolve(&input) {
        ResolverDecision::Commit(CommittedValue::ExistingOption(id)) => {
            assert_eq!(id, OptionId::from_raw(2));
        }
        other => panic!("expected Commit(ExistingOption), got {:?}", other),
    }
}

#[test]
fn input_confirm_ambiguous_rejects() {
    let resolver = TestResolver::handle();

    let options = vec![(OptionId::from_raw(1), "X"), (OptionId::from_raw(2), "X")];
    let entries: Vec<OptionEntry<'_>> = options
        .iter()
        .map(|(id, label)| OptionEntry { id: *id, label })
        .collect();

    let input = ResolverInput {
        input_text: Some("X"),
        highlighted_id: None,
        commit_source: CommitSource::InputConfirm,
        options: &entries,
    };

    assert!(matches!(
        resolver.resolve(&input),
        ResolverDecision::Reject(_)
    ));
}
