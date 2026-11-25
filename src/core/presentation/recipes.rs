use std::sync::Arc;

use crate::core::math::{Point, RectF};
use crate::core::scene::SceneNode;
use crate::core::text_engine::TextGenerationLease;

use super::*;

#[derive(Debug, Clone)]
pub(crate) struct PreparedOccurrence {
    pub source: Arc<CertifiedSource>,
    pub id: OccurrenceId,
    pub role: SourceRole,
    pub translation: Point,
    pub mask: Option<RectF>,
}

impl PreparedOccurrence {
    pub(crate) fn materialize(&self) -> Result<Arc<[SceneNode]>, PresentationFailure> {
        let paint = project_paint_values(&self.source.paint, self.id, self.role, self.translation)?;
        Ok(match self.mask {
            Some(mask) => apply_presentation_mask(paint, mask),
            None => paint,
        })
    }
}

/// Canonical generation references and continuation values remain separate.
/// This immutable value becomes a continuation only after App execution evidence.
#[derive(Debug, Clone)]
pub(crate) struct PreparedGroup {
    #[allow(dead_code, reason = "Retained prepared-group identity attribution")]
    pub ticket: PresentationTicket,
    pub occurrences: Vec<PreparedOccurrence>,
    pub canonical_generations: Vec<SourceGenerationId>,
    pub retained_generations: Vec<SourceGenerationId>,
    pub boundary: Option<f32>,
    pub terminal: bool,
    pub charge: PresentationCharge,
    pub exchange: Option<ExchangePath>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExchangePath {
    pub before: Vec<Arc<CertifiedSource>>,
    pub after: Vec<Arc<CertifiedSource>>,
    pub position: f32,
    pub axis: PresentationAxis,
}

fn coordinate(point: Point, axis: PresentationAxis) -> f32 {
    match axis {
        PresentationAxis::Horizontal => point.x,
        PresentationAxis::Vertical => point.y,
    }
}
fn leading(rect: RectF, axis: PresentationAxis) -> f32 {
    coordinate(Point::new(rect.x, rect.y), axis)
}
fn trailing(rect: RectF, axis: PresentationAxis) -> f32 {
    leading(rect, axis)
        + match axis {
            PresentationAxis::Horizontal => rect.width,
            PresentationAxis::Vertical => rect.height,
        }
}
fn along(value: f32, axis: PresentationAxis) -> Point {
    match axis {
        PresentationAxis::Horizontal => Point::new(value, 0.0),
        PresentationAxis::Vertical => Point::new(0.0, value),
    }
}
fn interpolate(from: f32, to: f32, p: f32) -> f32 {
    from + (to - from) * p
}
fn mask_interval(host: RectF, low: f32, high: f32, axis: PresentationAxis) -> RectF {
    let low = low.max(leading(host, axis));
    let high = high.min(trailing(host, axis)).max(low);
    match axis {
        PresentationAxis::Horizontal => RectF::new(low, host.y, high - low, host.height),
        PresentationAxis::Vertical => RectF::new(host.x, low, host.width, high - low),
    }
}
fn validate_text_generation(
    source: &CertifiedSource,
    font: Option<&TextGenerationLease>,
) -> Result<(), PresentationFailure> {
    if let Some(required) = &source.font {
        if !font.is_some_and(|current| required.same_generation(current)) {
            return Err(PresentationFailure::UnavailableResource);
        }
    }
    Ok(())
}
pub(crate) fn prepare_group(
    publication: &GroupPublication,
    current: &CurrentSources,
    font: Option<&TextGenerationLease>,
    host: RectF,
    limits: PresentationLimits,
) -> Result<Arc<PreparedGroup>, PresentationFailure> {
    let progress = publication.progress;
    if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
        return Err(PresentationFailure::RefusedSource);
    }
    if publication.first && progress != 0.0 {
        return Err(PresentationFailure::ExpiredFirstPublication);
    }
    valid_rect(host)?;
    let sources: Vec<_> = publication
        .historical
        .iter()
        .map(|previous| {
            let source = current
                .get(&previous.address)
                .ok_or(PresentationFailure::RefusedSource)?
                .clone()?;
            if source.host_path != previous.host_path
                || source.members != previous.members
                || source.scale_bits != previous.scale_bits
            {
                return Err(PresentationFailure::RefusedCoverage);
            }
            validate_text_generation(previous, font)?;
            Ok(source)
        })
        .collect::<Result<_, _>>()?;
    let canonical_generations: Vec<_> = sources.iter().map(|source| source.generation).collect();
    if publication
        .canonical_generations
        .as_ref()
        .is_some_and(|expected| *expected != canonical_generations)
    {
        return Err(PresentationFailure::RefusedSource);
    }
    let mut prepared = PreparedGroup {
        ticket: publication.ticket,
        occurrences: Vec::new(),
        canonical_generations,
        retained_generations: Vec::new(),
        boundary: None,
        terminal: progress == 1.0,
        charge: PresentationCharge::default(),
        exchange: None,
    };
    if !prepared.terminal {
        match &publication.request.composition {
            PresentationComposition::Exchange(recipe) => {
                prepare_exchange(publication, &sources, recipe, host, &mut prepared)?
            }
            PresentationComposition::Projections(projections) => {
                for (index, (projection, source)) in projections.iter().zip(&sources).enumerate() {
                    let translation = match projection.transform {
                        PresentationTransform::Identity => Point::zero(),
                        PresentationTransform::Translation { from, to } => Point::new(
                            interpolate(from.x, to.x, progress),
                            interpolate(from.y, to.y, progress),
                        ),
                        PresentationTransform::Affine { .. } => {
                            return Err(PresentationFailure::PendingCapability(
                                PendingPresentationCapability::Affine,
                            ));
                        }
                    };
                    let mask = match projection.mask {
                        PresentationMask::None => None,
                        PresentationMask::HardRect { from, to } => {
                            valid_rect(from)?;
                            valid_rect(to)?;
                            if to != host {
                                return Err(PresentationFailure::RefusedCoverage);
                            }
                            Some(RectF::new(
                                interpolate(from.x, to.x, progress),
                                interpolate(from.y, to.y, progress),
                                interpolate(from.width, to.width, progress),
                                interpolate(from.height, to.height, progress),
                            ))
                        }
                        PresentationMask::RoundedRect { .. } => {
                            return Err(PresentationFailure::PendingCapability(
                                PendingPresentationCapability::SoftMask,
                            ));
                        }
                    };
                    if let Some(mask) = mask {
                        valid_rect(mask)?;
                    }
                    if projection.isolated_opacity.is_some() {
                        return Err(PresentationFailure::PendingCapability(
                            PendingPresentationCapability::IsolatedOpacity,
                        ));
                    }
                    prepared.occurrences.push(PreparedOccurrence {
                        source: source.clone(),
                        id: publication.occurrences[index],
                        role: SourceRole::Current,
                        translation,
                        mask,
                    });
                }
            }
        }
    }
    if prepared.occurrences.len() > limits.occurrences {
        return Err(PresentationFailure::RefusedBudget);
    }
    let mut generations = Vec::new();
    for source in publication
        .historical
        .iter()
        .chain(sources.iter())
        .chain(prepared.occurrences.iter().map(|value| &value.source))
        .chain(
            prepared
                .exchange
                .iter()
                .flat_map(|path| path.before.iter().chain(&path.after)),
        )
        .chain(
            publication
                .continuation
                .iter()
                .flat_map(|group| group.occurrences.iter().map(|value| &value.source)),
        )
        .chain(publication.continuation.iter().flat_map(|group| {
            group
                .exchange
                .iter()
                .flat_map(|path| path.before.iter().chain(&path.after))
        }))
    {
        if !generations.contains(&source.generation) {
            validate_text_generation(source, font)?;
            generations.push(source.generation);
            prepared.charge.add(source.charge)?;
        }
    }
    // Materialized values coexist with source descriptors and the prior executed
    // continuation. Charge their complete reachable storage before copying it.
    for occurrence in &prepared.occurrences {
        // Conservative peak: source descriptors, projected values and temporary
        // container clones may coexist until immutable materialization finishes.
        for _ in 0..3 {
            prepared.charge.add(occurrence.source.charge)?;
        }
        let wrappers = if occurrence.mask.is_some() { 3 } else { 1 };
        let depth = occurrence
            .source
            .charge
            .depth
            .checked_add(wrappers)
            .and_then(|value| {
                occurrence
                    .source
                    .host_path
                    .len()
                    .checked_mul(3)
                    .and_then(|host| value.checked_add(host))
            })
            .ok_or(PresentationFailure::RefusedBudget)?;
        prepared.charge.add(PresentationCharge {
            nodes: wrappers,
            depth,
            edges: 4,
            cpu_bytes: std::mem::size_of::<PreparedOccurrence>()
                + wrappers * std::mem::size_of::<SceneNode>()
                + 64,
            ..Default::default()
        })?;
    }
    if generations.len() > limits.generations || !prepared.charge.within(limits) {
        return Err(PresentationFailure::RefusedBudget);
    }
    prepared.charge.add(PresentationCharge {
        cpu_bytes: std::mem::size_of::<PreparedGroup>()
            + 3 * std::mem::size_of::<GroupPublication>()
            + 512 * publication.historical.len(),
        edges: publication.historical.len() * 6,
        ..Default::default()
    })?;
    if !prepared.charge.within(limits) {
        return Err(PresentationFailure::RefusedBudget);
    }
    prepared.retained_generations = generations;
    Ok(Arc::new(prepared))
}

fn prepare_exchange(
    publication: &GroupPublication,
    current: &[Arc<CertifiedSource>],
    recipe: &ExchangeRecipe,
    host: RectF,
    prepared: &mut PreparedGroup,
) -> Result<(), PresentationFailure> {
    if let Some(previous) = publication
        .continuation
        .as_ref()
        .and_then(|value| value.exchange.as_ref())
    {
        return prepare_exchange_continuation(
            publication,
            current,
            recipe,
            host,
            previous,
            prepared,
        );
    }
    let old = &publication.historical[0];
    let new = &current[0];
    let axis = recipe.axis;
    let delta = trailing(new.bounds, axis) - trailing(old.bounds, axis);
    if delta == 0.0 || recipe.displaced.is_empty() {
        return Err(PresentationFailure::RefusedOwnership);
    }
    if leading(old.bounds, axis) != leading(new.bounds, axis) {
        return Err(PresentationFailure::RefusedOwnership);
    }
    let p = publication.progress;
    prepared.exchange = Some(ExchangePath {
        before: publication.historical.clone(),
        after: current.to_vec(),
        position: p,
        axis,
    });
    let initial_boundary = publication
        .continuation
        .as_ref()
        .and_then(|value| value.boundary)
        .unwrap_or(trailing(old.bounds, axis));
    let boundary = interpolate(initial_boundary, trailing(new.bounds, axis), p);
    prepared.boundary = Some(boundary);
    let travel = if delta > 0.0 {
        trailing(old.bounds, axis) - leading(old.bounds, axis)
    } else {
        trailing(new.bounds, axis) - leading(new.bounds, axis)
    };
    let split = leading(new.bounds, axis)
        + if delta > 0.0 {
            travel * (1.0 - p)
        } else {
            travel * p
        };
    let incoming_offset = if delta > 0.0 {
        travel * (1.0 - p)
    } else {
        -travel * (1.0 - p)
    };
    let outgoing_offset = if delta > 0.0 { -travel * p } else { travel * p };
    let incoming_mask = if delta > 0.0 {
        mask_interval(host, split, boundary, axis)
    } else {
        mask_interval(host, leading(new.bounds, axis), split, axis)
    };
    let outgoing_mask = if delta > 0.0 {
        mask_interval(host, leading(new.bounds, axis), split, axis)
    } else {
        mask_interval(host, split, boundary, axis)
    };

    prepared.occurrences.push(PreparedOccurrence {
        source: old.clone(),
        id: publication.occurrences[0],
        role: SourceRole::Historical,
        translation: along(outgoing_offset, axis),
        mask: Some(outgoing_mask),
    });
    prepared.occurrences.push(PreparedOccurrence {
        source: new.clone(),
        id: *publication.occurrences.last().unwrap(),
        role: SourceRole::Current,
        translation: along(incoming_offset, axis),
        mask: Some(incoming_mask),
    });

    for index in 1..current.len() {
        let old = &publication.historical[index];
        let new = &current[index];
        let canonical_delta = Point::new(new.bounds.x - old.bounds.x, new.bounds.y - old.bounds.y);
        let axis_delta = coordinate(canonical_delta, axis);
        let transverse = match axis {
            PresentationAxis::Horizontal => canonical_delta.y,
            PresentationAxis::Vertical => canonical_delta.x,
        };
        if axis_delta.signum() != delta.signum() || transverse.abs() > 0.001 {
            return Err(PresentationFailure::RefusedOwnership);
        }
        let from = publication
            .continuation
            .as_ref()
            .and_then(|previous| {
                previous
                    .occurrences
                    .iter()
                    .find(|value| value.source.address == new.address)
            })
            .map_or_else(
                || Point::new(-canonical_delta.x, -canonical_delta.y),
                |previous| {
                    Point::new(
                        previous.source.bounds.x + previous.translation.x - new.bounds.x,
                        previous.source.bounds.y + previous.translation.y - new.bounds.y,
                    )
                },
            );
        prepared.occurrences.push(PreparedOccurrence {
            source: new.clone(),
            id: publication.occurrences[index],
            role: SourceRole::Current,
            translation: Point::new(from.x * (1.0 - p), from.y * (1.0 - p)),
            mask: Some(mask_interval(host, boundary, trailing(host, axis), axis)),
        });
    }
    Ok(())
}

/// Exact inverse/continuation of the last executed path. Only the current
/// endpoint may be rebound; a different third endpoint settles canonically.
fn prepare_exchange_continuation(
    publication: &GroupPublication,
    current: &[Arc<CertifiedSource>],
    recipe: &ExchangeRecipe,
    host: RectF,
    previous: &ExchangePath,
    prepared: &mut PreparedGroup,
) -> Result<(), PresentationFailure> {
    if previous.axis != recipe.axis
        || current.len() != previous.before.len()
        || current
            .iter()
            .zip(&previous.before)
            .any(|(a, b)| a.address != b.address)
    {
        return Err(PresentationFailure::RefusedOwnership);
    }
    let matches = |candidate: &CertifiedSource| {
        current[0].bounds == candidate.bounds
            && paint_values_equal(&current[0].paint, &candidate.paint)
    };
    let target = if matches(&previous.before[0]) {
        0.0
    } else if matches(&previous.after[0]) {
        1.0
    } else {
        return Err(PresentationFailure::RefusedSource);
    };
    let q = interpolate(previous.position, target, publication.progress);
    let mut path_publication = publication.clone();
    path_publication.historical = previous.before.clone();
    path_publication.progress = q;
    path_publication.continuation = None;
    prepare_exchange(&path_publication, &previous.after, recipe, host, prepared)?;
    let endpoint = if target == 0.0 {
        &previous.before
    } else {
        &previous.after
    };
    for occurrence in &mut prepared.occurrences {
        let index = current
            .iter()
            .position(|source| source.address == occurrence.source.address)
            .ok_or(PresentationFailure::RefusedSource)?;
        if index == 0 {
            if occurrence.source.generation == endpoint[0].generation {
                occurrence.source = current[0].clone();
                occurrence.role = SourceRole::Current;
            } else {
                occurrence.role = SourceRole::Historical;
            }
        } else {
            let desired = Point::new(
                occurrence.source.bounds.x + occurrence.translation.x,
                occurrence.source.bounds.y + occurrence.translation.y,
            );
            if current[index].bounds != endpoint[index].bounds {
                return Err(PresentationFailure::RefusedCoverage);
            }
            occurrence.source = current[index].clone();
            occurrence.role = SourceRole::Current;
            occurrence.translation = Point::new(
                desired.x - current[index].bounds.x,
                desired.y - current[index].bounds.y,
            );
        }
    }
    Ok(())
}
