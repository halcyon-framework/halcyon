//! SPEC-HAL-199 §6.5-6.8: Selection Resolver Contract
//!
//! Defines the data-layer interface for commit resolution in selection controls
//! (ComboBox, ListPopup, etc.). All matching, filtering, ranking, and commit
//! policy logic lives here — never in widgets.
//!
//! Bound by SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution) and
//! §4.11 (Explicit Commit Authority).
//!
//! ## Widget Boundary
//!
//! Widgets import from this module:
//! - `ResolverHandle` (opaque, the only resolver type widgets hold)
//! - `ResolverDecision`, `CommittedValue`, `RejectionReason` (decision output)
//! - `ResolverInput`, `CommitSource`, `OptionId`, `OptionEntry` (input types)
//! - `LegacyPrefixPreview` (ordered presentation hints)
//!
//! Widgets do NOT import:
//! - `SelectionResolver` trait (application glue imports this directly)
//!
//! This separation is architectural law, not convenience.

// =============================================================================
// Option Identity
// =============================================================================

/// Stable, opaque option identifier.
///
/// Identity is adapter-provided. The resolver contract does not
/// generate, allocate, or manufacture option IDs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct OptionId(u64);

impl OptionId {
    /// Create from adapter-provided raw value.
    pub const fn from_raw(id: u64) -> Self {
        OptionId(id)
    }

    /// Get the raw u64 value.
    pub const fn raw(&self) -> u64 {
        self.0
    }
}

// =============================================================================
// Option Entry (Borrowed View)
// =============================================================================

/// Borrowed view of a selectable option.
///
/// Zero-allocation. Adapter owns the backing data.
/// Resolver reads label text; never allocates or copies it.
#[derive(Debug, Clone, Copy)]
pub struct OptionEntry<'a> {
    pub id: OptionId,
    pub label: &'a str,
}

/// Compatibility preview over the current ordered option snapshot. Replace
/// the cache when its owner replaces the snapshot. Queries leave it intact.
#[derive(Default)]
pub(crate) struct LegacyPrefixPreview {
    lowered_labels: Vec<String>,
}

impl LegacyPrefixPreview {
    pub(crate) fn replace_options(&mut self, options: &[(OptionId, String)]) {
        self.lowered_labels.clear();
        self.lowered_labels
            .extend(options.iter().map(|(_, label)| label.to_lowercase()));
    }

    pub(crate) fn first_highlight(
        &self,
        options: &[(OptionId, String)],
        query: &str,
    ) -> Option<OptionId> {
        if query.is_empty() {
            return None;
        }
        debug_assert_eq!(options.len(), self.lowered_labels.len());
        let query = query.to_lowercase();
        options
            .iter()
            .zip(&self.lowered_labels)
            .find(|(_, lowered)| lowered.starts_with(&query))
            .map(|((id, _), _)| *id)
    }

    pub(crate) fn ghost_suffix(
        &self,
        options: &[(OptionId, String)],
        query: &str,
        highlighted: Option<OptionId>,
    ) -> Option<String> {
        let highlighted = highlighted?;
        if query.is_empty() {
            return None;
        }
        debug_assert_eq!(options.len(), self.lowered_labels.len());
        let ((_, label), lowered) = options
            .iter()
            .zip(&self.lowered_labels)
            .find(|((id, _), _)| *id == highlighted)?;
        if !lowered.starts_with(&query.to_lowercase()) {
            return None;
        }
        // Preserve the compatibility offset in original Unicode scalars, even
        // when lowercasing changes the length of the matched prefix.
        let suffix: String = label.chars().skip(query.chars().count()).collect();
        (!suffix.is_empty()).then_some(suffix)
    }
}

// =============================================================================
// Commit Source
// =============================================================================

/// How the commit was triggered (SPEC-HAL-199 §6.6).
///
/// Two sources only. No booleans. No implicit disambiguation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommitSource {
    /// User selected a list item (click or keyboard nav + Enter).
    Selection,
    /// User pressed Enter with typed input.
    InputConfirm,
}

// =============================================================================
// Resolver Input
// =============================================================================

/// Everything the resolver receives to make a decision.
///
/// Signals are explicit and separate. The resolver sees raw input
/// text and highlighted option ID as independent signals — no
/// implicit disambiguation, no collapsed state.
#[derive(Debug, Clone, Copy)]
pub struct ResolverInput<'a> {
    /// Raw text input buffer (None if no text field active).
    pub input_text: Option<&'a str>,
    /// ID of the currently highlighted option (None if nothing highlighted).
    pub highlighted_id: Option<OptionId>,
    /// How the commit was triggered.
    pub commit_source: CommitSource,
    /// Full option list (adapter-owned, read-only borrowed view).
    pub options: &'a [OptionEntry<'a>],
}

// =============================================================================
// Resolver Decision (Output)
// =============================================================================

/// The value produced by a successful commit.
#[derive(Debug, Clone, PartialEq)]
pub enum CommittedValue {
    /// An existing option was selected.
    ExistingOption(OptionId),
    /// A custom value was committed (adapter policy allowed it).
    /// String allocation is intentional: custom values are new data.
    CustomValue(String),
}

/// Why the resolver rejected a commit.
#[derive(Debug, Clone, PartialEq)]
pub enum RejectionReason {
    /// Input does not match any option and custom values are disallowed.
    NoMatch,
    /// Input is empty.
    EmptyInput,
    /// Custom reason from adapter policy.
    Custom(String),
}

/// The resolver's decision (SPEC-HAL-199 §6.5-6.8).
///
/// Two states only. Commit or Reject.
/// The resolver is called on explicit commit actions and must decide.
#[derive(Debug, Clone, PartialEq)]
pub enum ResolverDecision {
    /// Commit this value.
    Commit(CommittedValue),
    /// Reject the commit.
    Reject(RejectionReason),
}

// =============================================================================
// Selection Resolver Trait
// =============================================================================

/// SPEC-HAL-199 §6.5-6.8: Selection resolver contract.
///
/// Resolvers are pure, deterministic, side-effect free functions
/// that decide what happens when a user commits in a selection control.
///
/// This is **adapter policy**, not widget behavior. Widgets call
/// resolve() through `ResolverHandle` and never implement this trait.
///
/// ## Invariants
///
/// - MUST be side-effect free (no I/O, no mutation, no logging)
/// - MUST be deterministic (same input → same output)
/// - MUST NOT mutate the option list
/// - MUST NOT access any UI state (focus, popup, scroll, geometry)
/// - MUST NOT depend on call ordering or history
pub trait SelectionResolver {
    fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision;
}

// =============================================================================
// Resolver Handle (Opaque Widget-Facing Wrapper)
// =============================================================================

/// Opaque handle to a selection resolver.
///
/// This is the ONLY resolver type widgets may hold or interact with.
/// Widgets see `ResolverHandle` + `ResolverDecision`. They never see
/// matching rules, normalization rules, or ranking logic.
///
/// Application glue creates `ResolverHandle` and passes it to widgets.
/// Widgets call `resolve()` and act on the `ResolverDecision`.
pub struct ResolverHandle {
    inner: Box<dyn SelectionResolver>,
}

impl ResolverHandle {
    /// Create a handle wrapping any resolver implementation.
    ///
    /// Called by application glue, not by widgets.
    pub fn new(resolver: impl SelectionResolver + 'static) -> Self {
        Self {
            inner: Box::new(resolver),
        }
    }

    /// Resolve a commit action. Widgets call this.
    pub fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
        self.inner.resolve(input)
    }
}
