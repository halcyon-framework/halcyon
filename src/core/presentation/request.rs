use crate::core::animation::Easing;
use crate::core::math::{Point, RectF};
use crate::utils::id::TreeWidgetId;

use super::{PendingPresentationCapability, PresentationFailure};

/// Addresses a paint source; none of these addresses creates an input identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PresentationSource {
    Widget(TreeWidgetId),
    Subtree(TreeWidgetId),
    Member {
        owner: TreeWidgetId,
        key: u64,
        lifetime: u64,
    },
}

impl PresentationSource {
    pub(crate) fn owner(self) -> TreeWidgetId {
        match self {
            Self::Widget(owner) | Self::Subtree(owner) | Self::Member { owner, .. } => owner,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationAxis {
    Horizontal,
    Vertical,
}

/// A recipe requests choreography. It is lowered into ordinary occurrence
/// projections; it owns neither playback nor canonical geometry.
#[derive(Debug, Clone, PartialEq)]
pub struct ExchangeRecipe {
    pub replacement: PresentationSource,
    pub displaced: Vec<PresentationSource>,
    pub axis: PresentationAxis,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresentationTransform {
    Identity,
    Translation { from: Point, to: Point },
    Affine { from: [f32; 6], to: [f32; 6] },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PresentationMask {
    None,
    HardRect { from: RectF, to: RectF },
    RoundedRect { from: RectF, to: RectF, radius: f32 },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationProjection {
    pub source: PresentationSource,
    pub transform: PresentationTransform,
    pub mask: PresentationMask,
    pub isolated_opacity: Option<(f32, f32)>,
}

/// Finite values only. General operators stay representable even when their
/// execution profile is not admitted by the current renderer.
#[derive(Debug, Clone, PartialEq)]
pub enum PresentationComposition {
    Exchange(ExchangeRecipe),
    Projections(Vec<PresentationProjection>),
}

impl PresentationComposition {
    fn retained_bytes(&self) -> Option<usize> {
        match self {
            Self::Exchange(value) => value
                .displaced
                .capacity()
                .checked_mul(std::mem::size_of::<PresentationSource>()),
            Self::Projections(values) => values
                .capacity()
                .checked_mul(std::mem::size_of::<PresentationProjection>()),
        }
    }
    fn source_count(&self) -> Option<usize> {
        match self {
            Self::Exchange(value) => value.displaced.len().checked_add(1),
            Self::Projections(values) => Some(values.len()),
        }
    }
    pub(crate) fn is_canonical_identity(&self) -> bool {
        matches!(self, Self::Projections(values) if !values.is_empty() && values.iter().all(|value| {
            (matches!(value.transform, PresentationTransform::Identity)
                || matches!(value.transform, PresentationTransform::Translation { from, to } if from == Point::zero() && to == Point::zero()))
                && value.mask == PresentationMask::None && value.isolated_opacity.is_none()
        }))
    }
    pub(crate) fn sources(&self) -> Vec<PresentationSource> {
        match self {
            Self::Exchange(recipe) => std::iter::once(recipe.replacement)
                .chain(recipe.displaced.iter().copied())
                .collect(),
            Self::Projections(values) => values.iter().map(|value| value.source).collect(),
        }
    }

    pub(crate) fn preliminary_admission(&self) -> Result<(), PresentationFailure> {
        use PendingPresentationCapability as Pending;
        if self
            .sources()
            .iter()
            .any(|source| matches!(source, PresentationSource::Member { .. }))
        {
            return Err(PresentationFailure::PendingCapability(
                Pending::MemberSource,
            ));
        }
        if let Self::Projections(values) = self {
            for value in values {
                if matches!(value.transform, PresentationTransform::Affine { .. }) {
                    return Err(PresentationFailure::PendingCapability(Pending::Affine));
                }
                if value.isolated_opacity.is_some() {
                    return Err(PresentationFailure::PendingCapability(
                        Pending::IsolatedOpacity,
                    ));
                }
                if matches!(value.mask, PresentationMask::RoundedRect { .. }) {
                    return Err(PresentationFailure::PendingCapability(Pending::SoftMask));
                }
                if let PresentationTransform::Translation { from, to } = value.transform {
                    if ![from.x, from.y, to.x, to.y]
                        .iter()
                        .all(|value| value.is_finite())
                    {
                        return Err(PresentationFailure::RefusedSource);
                    }
                    if to != Point::zero() {
                        return Err(PresentationFailure::RefusedOwnership);
                    }
                }
            }
        }
        Ok(())
    }
}

/// Runtime limits are supplied explicitly by the owning policy. There are no
/// framework timing, reduced-motion or historical-retention defaults.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationLimits {
    pub groups: usize,
    pub queued_requests: usize,
    pub sources: usize,
    pub source_widgets: usize,
    pub occurrences: usize,
    pub generations: usize,
    pub nodes: usize,
    pub depth: usize,
    pub dependency_edges: usize,
    pub cpu_bytes: usize,
    pub text_bytes: usize,
    pub font_bytes: usize,
    pub resource_bytes: usize,
    pub in_flight_bytes: usize,
    pub capture_nodes_per_frame: usize,
    pub max_duration_secs: f32,
    pub max_historical_age_ms: u64,
}

impl PresentationLimits {
    pub(crate) fn validate(&self) -> Result<(), PresentationFailure> {
        if [
            self.groups,
            self.queued_requests,
            self.sources,
            self.source_widgets,
            self.occurrences,
            self.generations,
            self.nodes,
            self.depth,
            self.dependency_edges,
            self.cpu_bytes,
            self.text_bytes,
            self.font_bytes,
            self.capture_nodes_per_frame,
        ]
        .contains(&0)
            || !self.max_duration_secs.is_finite()
            || self.max_duration_secs <= 0.0
            || self.max_historical_age_ms == 0
        {
            return Err(PresentationFailure::RefusedBudget);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PresentationMotionPolicy {
    pub enabled: bool,
    /// Explicit owner disposition of a reduced-motion preference.
    pub allow_nonessential_motion: bool,
    pub limits: PresentationLimits,
}

/// Opt-in custody of the finite source set starts at the next legal Update.
/// It is needed before replacement so previous paint is certified when produced.
#[derive(Debug, Clone, PartialEq)]
pub struct PresentationConfiguration {
    pub owner: TreeWidgetId,
    pub sources: Vec<PresentationSource>,
    pub policy: PresentationMotionPolicy,
}

impl PresentationConfiguration {
    pub(crate) fn validate(&self) -> Result<(), PresentationFailure> {
        self.policy.limits.validate()?;
        if self.sources.is_empty() || self.sources.len() > self.policy.limits.sources {
            return Err(PresentationFailure::RefusedBudget);
        }
        if self
            .sources
            .capacity()
            .checked_mul(std::mem::size_of::<PresentationSource>())
            .is_none_or(|bytes| bytes > self.policy.limits.cpu_bytes)
        {
            return Err(PresentationFailure::RefusedBudget);
        }
        for (index, source) in self.sources.iter().enumerate() {
            if self.sources[..index].contains(source) {
                return Err(PresentationFailure::RefusedSource);
            }
            if matches!(source, PresentationSource::Member { .. }) {
                return Err(PresentationFailure::PendingCapability(
                    PendingPresentationCapability::MemberSource,
                ));
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationRequestId(pub(crate) u64);
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PresentationGroupId(pub(crate) u64);

#[derive(Debug, Clone, PartialEq)]
pub struct PresentationRequest {
    /// Monotonic within the configured owner/policy revision.
    pub cause: u64,
    pub composition: PresentationComposition,
    pub duration_secs: f32,
    pub easing: Easing,
    /// Replace this live group, or start from canonical content if its bounded
    /// latest receipt already proves completion. Unknown groups are refused.
    pub replaces: Option<PresentationGroupId>,
    pub settlement: PresentationSettlement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationSettlement {
    Canonical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationTicket {
    pub request: PresentationRequestId,
    pub group: PresentationGroupId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationOutcome {
    Reserved,
    Admitted,
    Completed,
    Cancelled,
    SettledCanonical,
    Disabled,
    NoOp,
    Refused(PresentationFailure),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PresentationReceipt {
    pub ticket: PresentationTicket,
    pub outcome: PresentationOutcome,
}

/// The existing tree's deferred ingress. Playback/control stays in App Update.
pub(crate) struct PresentationInbox {
    pub configuration: PresentationConfiguration,
    pub revision: u64,
    pub requests: Vec<(PresentationTicket, PresentationRequest)>,
    pub receipts: Vec<PresentationReceipt>,
    last_request: Option<(PresentationTicket, PresentationRequest)>,
}

impl PresentationInbox {
    pub(crate) fn retained_bytes(&self) -> Result<usize, PresentationFailure> {
        let mut bytes = std::mem::size_of::<Self>();
        for (capacity, size) in [
            (
                self.configuration.sources.capacity(),
                2 * std::mem::size_of::<PresentationSource>(),
            ),
            (
                self.requests.capacity(),
                std::mem::size_of::<(PresentationTicket, PresentationRequest)>(),
            ),
            (
                self.receipts.capacity(),
                std::mem::size_of::<PresentationReceipt>(),
            ),
        ] {
            bytes = capacity
                .checked_mul(size)
                .and_then(|value| bytes.checked_add(value))
                .ok_or(PresentationFailure::RefusedBudget)?;
        }
        for request in self
            .requests
            .iter()
            .map(|(_, request)| request)
            .chain(self.last_request.iter().map(|(_, request)| request))
        {
            bytes = request
                .composition
                .retained_bytes()
                .and_then(|value| bytes.checked_add(value))
                .ok_or(PresentationFailure::RefusedBudget)?;
        }
        Ok(bytes)
    }
    pub(crate) fn new(
        configuration: PresentationConfiguration,
    ) -> Result<Self, PresentationFailure> {
        configuration.validate()?;
        Ok(Self {
            configuration,
            revision: 1,
            requests: Vec::new(),
            receipts: Vec::new(),
            last_request: None,
        })
    }

    pub(crate) fn configure(
        &mut self,
        configuration: PresentationConfiguration,
    ) -> Result<(), PresentationFailure> {
        configuration.validate()?;
        if self.configuration == configuration {
            return Ok(());
        }
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(PresentationFailure::RefusedBudget)?;
        for (ticket, _) in std::mem::take(&mut self.requests) {
            self.record(PresentationReceipt {
                ticket,
                outcome: PresentationOutcome::Cancelled,
            });
        }
        self.configuration = configuration;
        let surplus = self
            .receipts
            .len()
            .saturating_sub(self.configuration.policy.limits.queued_requests);
        self.receipts.drain(..surplus);
        self.revision = revision;
        self.last_request = None;
        Ok(())
    }

    pub(crate) fn request(
        &mut self,
        request: PresentationRequest,
    ) -> Result<PresentationTicket, PresentationFailure> {
        if let Some((ticket, previous)) = &self.last_request {
            if request == *previous {
                return Ok(*ticket);
            }
            if request.cause <= previous.cause {
                return Err(PresentationFailure::RefusedSource);
            }
        }
        let limits = self.configuration.policy.limits;
        if self.requests.len() >= limits.queued_requests {
            return Err(PresentationFailure::RefusedBudget);
        }
        if request
            .composition
            .source_count()
            .is_none_or(|count| count == 0 || count > limits.sources)
        {
            return Err(PresentationFailure::RefusedBudget);
        }
        let control_peak = request
            .composition
            .retained_bytes()
            .and_then(|bytes| bytes.checked_mul(2))
            .and_then(|bytes| {
                bytes.checked_add(
                    2 * std::mem::size_of::<(PresentationTicket, PresentationRequest)>(),
                )
            })
            .and_then(|bytes| {
                self.retained_bytes()
                    .ok()
                    .and_then(|retained| bytes.checked_add(retained))
            });
        if control_peak.is_none_or(|bytes| bytes > limits.cpu_bytes) {
            return Err(PresentationFailure::RefusedBudget);
        }
        request.composition.preliminary_admission()?;
        let sources = request.composition.sources();
        if sources.is_empty() || sources.len() > limits.sources {
            return Err(PresentationFailure::RefusedBudget);
        }
        if sources
            .iter()
            .any(|source| !self.configuration.sources.contains(source))
        {
            return Err(PresentationFailure::RefusedOwnership);
        }
        if sources
            .iter()
            .enumerate()
            .any(|(index, source)| sources[..index].contains(source))
        {
            return Err(PresentationFailure::RefusedOwnership);
        }
        if request.cause == 0
            || !request.duration_secs.is_finite()
            || request.duration_secs < 0.0
            || request.duration_secs > limits.max_duration_secs
        {
            return Err(PresentationFailure::RefusedSource);
        }
        let ticket = PresentationTicket {
            request: PresentationRequestId(super::next_value_id()?),
            group: PresentationGroupId(super::next_value_id()?),
        };
        self.last_request = Some((ticket, request.clone()));
        self.requests.push((ticket, request));
        Ok(ticket)
    }

    pub(crate) fn record(&mut self, receipt: PresentationReceipt) {
        if let Some(old) = self
            .receipts
            .iter_mut()
            .find(|value| value.ticket == receipt.ticket)
        {
            *old = receipt;
            return;
        }
        if self.receipts.len() >= self.configuration.policy.limits.queued_requests {
            self.receipts.remove(0);
        }
        self.receipts.push(receipt);
    }
}
