use std::collections::HashMap;
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

use crate::core::animation::{Animation, AnimationSample, TransitionHandle};
use crate::core::transition::TransitionLayer;

use super::*;

static NEXT_PRESENTATION_VALUE_ID: AtomicU64 = AtomicU64::new(1);

pub(crate) fn next_value_id() -> Result<u64, PresentationFailure> {
    NEXT_PRESENTATION_VALUE_ID
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
        .map_err(|_| PresentationFailure::RefusedBudget)
}

pub(crate) type CurrentSources =
    HashMap<PresentationSource, Result<Arc<CertifiedSource>, PresentationFailure>>;

#[derive(Clone)]
pub(crate) struct GroupPublication {
    pub ticket: PresentationTicket,
    pub request: PresentationRequest,
    pub progress: f32,
    pub first: bool,
    pub historical: Vec<Arc<CertifiedSource>>,
    pub occurrences: Vec<OccurrenceId>,
    pub canonical_generations: Option<Vec<SourceGenerationId>>,
    pub continuation: Option<Arc<PreparedGroup>>,
    pub capture_origin_ms: u64,
    pub policy_revision: u64,
}

struct GroupControl {
    publication: GroupPublication,
    handle: TransitionHandle,
    last_progress: Option<u32>,
    last_executed: Option<Arc<PreparedGroup>>,
    last_execution: Option<PresentationExecution>,
}

#[derive(Clone)]
pub(crate) struct GroupObservation {
    pub ticket: PresentationTicket,
    pub prepared: Result<Arc<PreparedGroup>, PresentationFailure>,
    pub execution: Option<PresentationExecution>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct PresentationExecution {
    #[allow(dead_code, reason = "Retained exact frame execution attribution")]
    pub attempt: crate::core::frame_realization::AppFrameAttemptId,
    #[allow(dead_code, reason = "Retained exact frame execution attribution")]
    pub surface: crate::compositor::SurfaceId,
    #[allow(dead_code, reason = "Retained exact frame execution attribution")]
    pub epoch: crate::renderer::FrameEpoch,
}

#[derive(Default)]
pub(crate) struct PresentationCoordinator {
    groups: Vec<GroupControl>,
    configuration_revision: u64,
    /// At most one latest outcome per bounded group. Written as observation by
    /// App; consumed as control only at the following Update boundary.
    observations: Vec<GroupObservation>,
}

pub(crate) struct UpdatePublication {
    pub groups: Vec<GroupPublication>,
    pub changed: bool,
}

/// On-demand detached inspection. It contains identities and values only;
/// querying it retains no source, resource lease, widget or Timeline handle.
#[cfg(test)]
#[derive(Debug)]
pub(crate) struct PresentationInspection {
    pub ticket: PresentationTicket,
    pub cause: u64,
    pub policy_revision: u64,
    pub progress: f32,
    pub reserved: bool,
    pub previous_canonical: Vec<(
        PresentationSource,
        SourceGenerationId,
        crate::core::math::RectF,
    )>,
    pub current_canonical: Vec<(
        PresentationSource,
        SourceGenerationId,
        crate::core::math::RectF,
    )>,
    pub occurrences: Vec<(
        OccurrenceId,
        SourceGenerationId,
        SourceRole,
        crate::core::math::Point,
        Option<crate::core::math::RectF>,
    )>,
    pub charge: PresentationCharge,
    pub execution: Option<PresentationExecution>,
    pub continuation_execution: Option<PresentationExecution>,
    pub frame_failure: Option<PresentationFailure>,
}

impl PresentationCoordinator {
    #[cfg(test)]
    pub(crate) fn inspect(&self) -> Vec<PresentationInspection> {
        self.groups
            .iter()
            .map(|group| {
                let publication = &group.publication;
                let frame = self
                    .observations
                    .iter()
                    .find(|value| value.ticket == publication.ticket);
                let prepared = frame
                    .and_then(|value| value.prepared.as_ref().ok())
                    .or(group.last_executed.as_ref());
                PresentationInspection {
                    ticket: publication.ticket,
                    cause: publication.request.cause,
                    policy_revision: publication.policy_revision,
                    progress: publication.progress,
                    reserved: publication.first,
                    previous_canonical: publication
                        .historical
                        .iter()
                        .map(|source| (source.address, source.generation, source.bounds))
                        .collect(),
                    current_canonical: Vec::new(),
                    occurrences: prepared.map_or_else(Vec::new, |value| {
                        value
                            .occurrences
                            .iter()
                            .map(|value| {
                                (
                                    value.id,
                                    value.source.generation,
                                    value.role,
                                    value.translation,
                                    value.mask,
                                )
                            })
                            .collect()
                    }),
                    charge: prepared.map_or_else(PresentationCharge::default, |value| value.charge),
                    execution: frame.and_then(|value| value.execution),
                    continuation_execution: group.last_execution,
                    frame_failure: frame.and_then(|value| value.prepared.as_ref().err().copied()),
                }
            })
            .collect()
    }
    pub(crate) fn observe(&mut self, observations: Vec<GroupObservation>) {
        // App owns this bounded immutable outcome mailbox, not group control.
        self.observations = observations;
    }

    pub(crate) fn record_execution(&mut self, execution: PresentationExecution) {
        self.observations = self
            .observations
            .iter()
            .map(|observation| GroupObservation {
                ticket: observation.ticket,
                prepared: observation.prepared.clone(),
                execution: Some(execution),
            })
            .collect();
    }

    pub(crate) fn needs_reconciliation(&self) -> bool {
        !self.observations.is_empty()
    }

    pub(crate) fn update(
        &mut self,
        inbox: &mut PresentationInbox,
        sources: &CurrentSources,
        timeline: &mut TransitionLayer,
        now_ms: u64,
        valid_owner: impl Fn(PresentationSource) -> bool,
        interaction_permits_motion: bool,
    ) -> UpdatePublication {
        crate::core::rect_authority::assert_update_mutation_authorized("presentation coordination");
        let mut changed = false;
        if self.configuration_revision != inbox.revision {
            for group in self.groups.drain(..) {
                timeline.cancel_animation(group.handle);
                inbox.record(PresentationReceipt {
                    ticket: group.publication.ticket,
                    outcome: PresentationOutcome::Cancelled,
                });
            }
            self.configuration_revision = inbox.revision;
            self.observations.clear();
            changed = true;
        }

        for observation in self.observations.drain(..) {
            let Some(index) = self
                .groups
                .iter()
                .position(|group| group.publication.ticket == observation.ticket)
            else {
                continue;
            };
            let outcome = match &observation.prepared {
                Ok(prepared) if observation.execution.is_some() => {
                    if prepared.terminal {
                        Some(PresentationOutcome::Completed)
                    } else {
                        let group = &mut self.groups[index];
                        group.publication.first = false;
                        group.publication.canonical_generations =
                            Some(prepared.canonical_generations.clone());
                        group.last_executed = Some(prepared.clone());
                        group.last_execution = observation.execution;
                        inbox.record(PresentationReceipt {
                            ticket: observation.ticket,
                            outcome: PresentationOutcome::Admitted,
                        });
                        None
                    }
                }
                Ok(_) => Some(PresentationOutcome::Refused(
                    PresentationFailure::UnavailableResource,
                )),
                Err(failure) => Some(PresentationOutcome::Refused(*failure)),
            };
            if let Some(outcome) = outcome {
                let group = self.groups.remove(index);
                timeline.cancel_animation(group.handle);
                inbox.record(PresentationReceipt {
                    ticket: observation.ticket,
                    outcome,
                });
                changed = true;
            }
        }

        let policy = inbox.configuration.policy;
        let mut index = 0;
        while index < self.groups.len() {
            let publication = &self.groups[index].publication;
            let disabled = !policy.enabled || !policy.allow_nonessential_motion;
            let expired = now_ms.saturating_sub(publication.capture_origin_ms)
                > policy.limits.max_historical_age_ms;
            if disabled
                || expired
                || !interaction_permits_motion
                || publication
                    .historical
                    .iter()
                    .any(|source| !valid_owner(source.address))
            {
                let group = self.groups.remove(index);
                timeline.cancel_animation(group.handle);
                inbox.record(PresentationReceipt {
                    ticket: group.publication.ticket,
                    outcome: if disabled {
                        PresentationOutcome::Disabled
                    } else {
                        PresentationOutcome::SettledCanonical
                    },
                });
                changed = true;
            } else {
                index += 1;
            }
        }

        let requests = std::mem::take(&mut inbox.requests);
        for (ticket, request) in requests {
            let replaced = request
                .replaces
                .and_then(|id| {
                    self.groups
                        .iter()
                        .position(|group| group.publication.ticket.group == id)
                })
                .map(|index| self.groups.remove(index));
            if let Some(previous) = &replaced {
                timeline.cancel_animation(previous.handle);
                inbox.record(PresentationReceipt {
                    ticket: previous.publication.ticket,
                    outcome: PresentationOutcome::Cancelled,
                });
                changed = true;
            }
            let result = (|| {
                if request.replaces.is_some()
                    && replaced.is_none()
                    && !inbox.receipts.iter().any(|receipt| {
                        Some(receipt.ticket.group) == request.replaces
                            && receipt.outcome == PresentationOutcome::Completed
                    })
                {
                    return Err(PresentationFailure::RefusedOwnership);
                }
                if !policy.enabled || !policy.allow_nonessential_motion {
                    return Ok(PresentationOutcome::Disabled);
                }
                if request.duration_secs == 0.0 {
                    return Ok(PresentationOutcome::SettledCanonical);
                }
                if !interaction_permits_motion {
                    return Err(PresentationFailure::RefusedOwnership);
                }
                if request.composition.is_canonical_identity() {
                    return Ok(PresentationOutcome::NoOp);
                }
                let requested_sources = request.composition.sources();
                let historical: Vec<_> = requested_sources
                    .iter()
                    .map(|address| {
                        if !valid_owner(*address) {
                            return Err(PresentationFailure::RefusedSource);
                        }
                        sources
                            .get(address)
                            .ok_or(PresentationFailure::RefusedSource)?
                            .clone()
                    })
                    .collect::<Result<_, _>>()?;
                if historical
                    .iter()
                    .map(|source| source.members.len())
                    .sum::<usize>()
                    > policy.limits.source_widgets
                {
                    return Err(PresentationFailure::RefusedBudget);
                }
                for (index, source) in historical.iter().enumerate() {
                    if historical[..index]
                        .iter()
                        .any(|other| other.members.iter().any(|id| source.members.contains(id)))
                    {
                        return Err(PresentationFailure::RefusedOwnership);
                    }
                }
                for group in &self.groups {
                    if group.publication.historical.iter().any(|old| {
                        historical
                            .iter()
                            .any(|new| old.members.iter().any(|owner| new.members.contains(owner)))
                    }) {
                        return Err(PresentationFailure::RefusedOwnership);
                    }
                }
                if self.groups.len() >= policy.limits.groups {
                    return Err(PresentationFailure::RefusedBudget);
                }
                let continuation = replaced
                    .as_ref()
                    .and_then(|group| group.last_executed.clone());
                if replaced.is_some() && continuation.is_none() {
                    return Err(PresentationFailure::RefusedSource);
                }
                let occurrences = (0..historical
                    .len()
                    .checked_add(1)
                    .ok_or(PresentationFailure::RefusedBudget)?)
                    .map(|_| next_value_id().map(OccurrenceId::new))
                    .collect::<Result<Vec<_>, _>>()?;
                if occurrences.len() > policy.limits.occurrences {
                    return Err(PresentationFailure::RefusedBudget);
                }
                let handle = timeline
                    .animation_context()
                    .start(Animation::new(
                        0.0f32,
                        1.0f32,
                        request.duration_secs,
                        request.easing,
                    ))
                    .map_err(|_| PresentationFailure::RefusedBudget)?;
                let capture_origin_ms = replaced
                    .as_ref()
                    .map_or(now_ms, |group| group.publication.capture_origin_ms);
                self.groups.push(GroupControl {
                    publication: GroupPublication {
                        ticket,
                        request: request.clone(),
                        progress: 0.0,
                        first: true,
                        historical,
                        occurrences,
                        canonical_generations: None,
                        continuation,
                        capture_origin_ms,
                        policy_revision: inbox.revision,
                    },
                    handle,
                    last_progress: None,
                    last_executed: None,
                    last_execution: None,
                });
                changed = true;
                Ok(PresentationOutcome::Reserved)
            })();
            inbox.record(PresentationReceipt {
                ticket,
                outcome: result.unwrap_or_else(PresentationOutcome::Refused),
            });
        }

        let mut publications = Vec::with_capacity(self.groups.len());
        let mut index = 0;
        while index < self.groups.len() {
            let group = &mut self.groups[index];
            match timeline.timeline().sample::<f32>(group.handle) {
                AnimationSample::Active(progress) | AnimationSample::Completed(progress) => {
                    changed |= group.last_progress != Some(progress.to_bits());
                    group.last_progress = Some(progress.to_bits());
                    group.publication.progress = progress;
                    publications.push(group.publication.clone());
                    index += 1;
                }
                _ => {
                    let group = self.groups.remove(index);
                    timeline.cancel_animation(group.handle);
                    inbox.record(PresentationReceipt {
                        ticket: group.publication.ticket,
                        outcome: PresentationOutcome::Cancelled,
                    });
                    changed = true;
                }
            }
        }
        UpdatePublication {
            groups: publications,
            changed,
        }
    }
}
