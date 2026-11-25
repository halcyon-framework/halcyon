//! SPEC-HAL-199 §6.8: Default Selection Resolver (Adapter Policy)
//!
//! Ships with Halcyon as a baseline adapter policy. Applications may
//! substitute any resolver satisfying the `SelectionResolver` trait.
//!
//! This is adapter policy, not widget behavior. Widgets are oblivious
//! to the resolver implementation. No widget behavior changes based on
//! which resolver is active.

use super::selection_resolver::{
    CommitSource, CommittedValue, OptionEntry, OptionId, RejectionReason, ResolverDecision,
    ResolverInput, SelectionResolver,
};

// =============================================================================
// Matching Policy
// =============================================================================

/// Adapter-level matching policy (SPEC-HAL-199 §6.8).
///
/// This is replaceable adapter policy. It governs how the default
/// resolver matches typed input against option labels.
///
/// Widgets are oblivious to which policy is active.
/// No widget behavior changes based on policy choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchingPolicy {
    /// Case-folded exact match. Input must match full label (case-insensitive).
    PolicyCaseFoldExact,
    /// Case-folded prefix match. Input must match label prefix (case-insensitive).
    PolicyCaseFoldPrefix,
}

// =============================================================================
// Default Selection Resolver
// =============================================================================

/// SPEC-HAL-199 §6.8: Default selection resolver (adapter policy, replaceable).
///
/// Ships with Halcyon as a baseline adapter policy. Applications may
/// substitute any resolver satisfying the `SelectionResolver` trait.
///
/// This is adapter policy, not widget behavior. Widgets are oblivious
/// to the resolver implementation. No widget behavior changes based on
/// which resolver is active.
///
/// ## Decision table
///
/// | CommitSource | Highlighted | Input text | Match? | Custom allowed? | Decision |
/// |---|---|---|---|---|---|
/// | Selection | Some(id) | _ | _ | _ | Commit(ExistingOption(id)) |
/// | Selection | None | _ | _ | _ | Reject(NoMatch) |
/// | InputConfirm | _ | None/empty | _ | _ | Reject(EmptyInput) |
/// | InputConfirm | _ | Some(text) | Yes(id) | _ | Commit(ExistingOption(id)) |
/// | InputConfirm | _ | Some(text) | No | true | Commit(CustomValue(text)) |
/// | InputConfirm | _ | Some(text) | No | false | Reject(NoMatch) |
pub struct DefaultSelectionResolver {
    /// Whether custom values (not in the option list) are accepted.
    pub allow_custom_values: bool,
    /// How to match input text against option labels.
    pub matching: MatchingPolicy,
}

impl DefaultSelectionResolver {
    /// Strict policy: case-folded exact match, no custom values.
    pub fn strict() -> Self {
        Self {
            allow_custom_values: false,
            matching: MatchingPolicy::PolicyCaseFoldExact,
        }
    }

    /// Permissive policy: case-folded prefix match, custom values allowed.
    pub fn permissive() -> Self {
        Self {
            allow_custom_values: true,
            matching: MatchingPolicy::PolicyCaseFoldPrefix,
        }
    }

    fn find_match(&self, text: &str, options: &[OptionEntry<'_>]) -> Option<OptionId> {
        let text_lower = text.to_lowercase();
        match self.matching {
            MatchingPolicy::PolicyCaseFoldExact => options
                .iter()
                .find(|o| o.label.to_lowercase() == text_lower)
                .map(|o| o.id),
            MatchingPolicy::PolicyCaseFoldPrefix => options
                .iter()
                .find(|o| o.label.to_lowercase().starts_with(&text_lower))
                .map(|o| o.id),
        }
    }
}

impl Default for DefaultSelectionResolver {
    fn default() -> Self {
        Self::strict()
    }
}

impl SelectionResolver for DefaultSelectionResolver {
    fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
        match input.commit_source {
            CommitSource::Selection => match input.highlighted_id {
                Some(id) => ResolverDecision::Commit(CommittedValue::ExistingOption(id)),
                None => ResolverDecision::Reject(RejectionReason::NoMatch),
            },
            CommitSource::InputConfirm => {
                let text = match input.input_text {
                    Some(t) if !t.is_empty() => t,
                    _ => return ResolverDecision::Reject(RejectionReason::EmptyInput),
                };

                if let Some(matched_id) = self.find_match(text, input.options) {
                    return ResolverDecision::Commit(CommittedValue::ExistingOption(matched_id));
                }

                if self.allow_custom_values {
                    ResolverDecision::Commit(CommittedValue::CustomValue(text.to_string()))
                } else {
                    ResolverDecision::Reject(RejectionReason::NoMatch)
                }
            }
        }
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::selection_resolver::ResolverHandle;

    fn test_options() -> Vec<(OptionId, String)> {
        vec![
            (OptionId::from_raw(1), "Apple".to_string()),
            (OptionId::from_raw(2), "Banana".to_string()),
            (OptionId::from_raw(3), "Cherry".to_string()),
        ]
    }

    fn entries_from(backing: &[(OptionId, String)]) -> Vec<OptionEntry<'_>> {
        backing
            .iter()
            .map(|(id, label)| OptionEntry { id: *id, label })
            .collect()
    }

    #[test]
    fn test_selection_commit_with_highlighted() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: None,
            highlighted_id: Some(OptionId::from_raw(2)),
            commit_source: CommitSource::Selection,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::ExistingOption(OptionId::from_raw(2)))
        );
    }

    #[test]
    fn test_selection_commit_without_highlighted() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: None,
            highlighted_id: None,
            commit_source: CommitSource::Selection,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(decision, ResolverDecision::Reject(RejectionReason::NoMatch));
    }

    #[test]
    fn test_input_confirm_exact_match() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("Banana"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::ExistingOption(OptionId::from_raw(2)))
        );
    }

    #[test]
    fn test_input_confirm_case_insensitive() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("APPLE"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::ExistingOption(OptionId::from_raw(1)))
        );
    }

    #[test]
    fn test_input_confirm_no_match_custom_disallowed() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("Mango"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(decision, ResolverDecision::Reject(RejectionReason::NoMatch));
    }

    #[test]
    fn test_input_confirm_no_match_custom_allowed() {
        let resolver = DefaultSelectionResolver::permissive();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("Mango"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::CustomValue("Mango".to_string()))
        );
    }

    #[test]
    fn test_input_confirm_empty() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some(""),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Reject(RejectionReason::EmptyInput)
        );
    }

    #[test]
    fn test_input_confirm_none_input() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: None,
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Reject(RejectionReason::EmptyInput)
        );
    }

    #[test]
    fn test_prefix_policy_match() {
        let resolver = DefaultSelectionResolver::permissive();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("App"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::ExistingOption(OptionId::from_raw(1)))
        );
    }

    #[test]
    fn test_prefix_policy_no_match() {
        let resolver = DefaultSelectionResolver {
            allow_custom_values: false,
            matching: MatchingPolicy::PolicyCaseFoldPrefix,
        };
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("xyz"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = resolver.resolve(&input);
        assert_eq!(decision, ResolverDecision::Reject(RejectionReason::NoMatch));
    }

    #[test]
    fn test_resolver_handle_opacity() {
        let resolver = DefaultSelectionResolver::strict();
        let handle = ResolverHandle::new(resolver);
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("Cherry"),
            highlighted_id: None,
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision = handle.resolve(&input);
        assert_eq!(
            decision,
            ResolverDecision::Commit(CommittedValue::ExistingOption(OptionId::from_raw(3)))
        );
    }

    #[test]
    fn test_determinism() {
        let resolver = DefaultSelectionResolver::strict();
        let backing = test_options();
        let entries = entries_from(&backing);
        let input = ResolverInput {
            input_text: Some("Banana"),
            highlighted_id: Some(OptionId::from_raw(1)),
            commit_source: CommitSource::InputConfirm,
            options: &entries,
        };
        let decision1 = resolver.resolve(&input);
        let decision2 = resolver.resolve(&input);
        assert_eq!(decision1, decision2);
    }
}
