use super::*;
use crate::compositor::SurfaceId;
use crate::core::frame_realization::AppFrameAttemptId;
use crate::core::presentation::*;
use crate::core::text_engine::TextGenerationLease;
use crate::core::widget::Widget;
use std::collections::HashSet;

/// Owned by the existing App retained boundary. No widget, clock or semantic
/// sidecar is retained here. Allocation occurs only after explicit opt-in.
#[derive(Default)]
pub(super) struct PresentationRuntime {
    coordinator: PresentationCoordinator,
    sources: CurrentSources,
    configuration: Option<PresentationConfiguration>,
    revision: u64,
    publication: Option<UpdatePublication>,
    pub(super) claimed: HashSet<TreeWidgetId>,
    target: Option<(AppFrameAttemptId, SurfaceId)>,
    pending_execution: Option<(AppFrameAttemptId, SurfaceId)>,
    pub(super) changed: bool,
    control_bytes: usize,
    #[cfg(test)]
    capture_visits: usize,
}

impl PresentationRuntime {
    #[cfg(test)]
    pub(super) fn inspect(&self) -> Vec<PresentationInspection> {
        let mut values = self.coordinator.inspect();
        for value in &mut values {
            value.current_canonical = value
                .previous_canonical
                .iter()
                .filter_map(|(address, _, _)| {
                    self.sources
                        .get(address)
                        .and_then(|value| value.as_ref().ok())
                        .map(|source| (source.address, source.generation, source.bounds))
                })
                .collect();
        }
        values
    }
    pub(super) fn needs_reconciliation(&self) -> bool {
        self.coordinator.needs_reconciliation()
    }

    pub(super) fn update(
        &mut self,
        tree: &mut WidgetTree,
        timeline: &mut TransitionLayer,
        layout_transitions: &mut ActiveLayoutTransitions,
        now_ms: u64,
        interaction_permits_motion: bool,
    ) {
        let Some(mut inbox) = tree.presentation_inbox.take() else {
            return;
        };
        if self.revision != inbox.revision {
            self.sources.clear();
            self.revision = inbox.revision;
            self.configuration = Some(inbox.configuration.clone());
            // Certification starts with ordinary producer paint in this frame.
            for source in &inbox.configuration.sources {
                if let Some(node) = tree.find_by_id(source.owner()) {
                    match source {
                        PresentationSource::Subtree(_) => tree.mark_subtree_paint_dirty(node),
                        _ => tree.mark_paint_dirty(node),
                    }
                }
            }
        }
        let owner = tree.find_by_id(inbox.configuration.owner);
        let source_is_valid = |source: PresentationSource| {
            tree.find_by_id(source.owner())
                .zip(owner)
                .is_some_and(|(node, owner)| tree.is_descendant_or_same(node, owner))
        };
        let publication = self.coordinator.update(
            &mut inbox,
            &self.sources,
            timeline,
            now_ms,
            &source_is_valid,
            interaction_permits_motion && owner.is_some(),
        );
        // SPEC-HAL-354: current custody eviction cannot depend on another paint.
        self.sources.retain(|source, _| source_is_valid(*source));
        self.claimed.clear();
        for group in &publication.groups {
            for source in &group.historical {
                self.claimed.extend(source.members.iter().copied());
            }
        }
        // A transformed ancestor would affect these leaves too. The same engine
        // arbitration excludes it; canonical geometry remains untouched.
        if !self.claimed.is_empty() {
            let mut arbitration = self.claimed.clone();
            for id in &self.claimed {
                let mut node = tree.find_by_id(*id).and_then(|node| tree.parent(node));
                while let Some(parent) = node {
                    if let Some(value) = tree.node(parent) {
                        arbitration.insert(value.id);
                    }
                    node = tree.parent(parent);
                }
            }
            self.claimed = arbitration;
            layout_transitions.yield_to_presentation(&self.claimed, timeline);
        }
        self.changed = publication.changed;
        self.publication = Some(publication);
        self.control_bytes = inbox.retained_bytes().unwrap_or(usize::MAX);
        tree.presentation_inbox = Some(inbox);
    }

    pub(super) fn expire_frame(&mut self) {
        let observations = self
            .publication
            .as_ref()
            .into_iter()
            .flat_map(|value| &value.groups)
            .map(|group| GroupObservation {
                ticket: group.ticket,
                prepared: Err(PresentationFailure::ExpiredFirstPublication),
                execution: None,
            })
            .collect();
        self.coordinator.observe(observations);
    }

    pub(super) fn submitted(
        &mut self,
        attempt: AppFrameAttemptId,
        surface: SurfaceId,
        epoch: crate::renderer::FrameEpoch,
    ) {
        // Called only after compile, output creation and submit_and_present for
        // this exact App attempt/surface. It makes no physical-display claim.
        if self.pending_execution == Some((attempt, surface)) {
            self.coordinator.record_execution(PresentationExecution {
                attempt,
                surface,
                epoch,
            });
            self.pending_execution = None;
        }
    }

    pub(super) fn compose(
        &mut self,
        tree: &WidgetTree,
        cache: &RetainedSceneCache,
        paint_root: NodeId,
        root: &RetainedWidgetScene,
        text: &dyn TextEngine,
        scale: f32,
        target: Option<CustomRenderTargetSnapshot>,
        interpolated: &InterpolatedVisualRects,
        appearance: Option<&FrameworkAppearanceSnapshot>,
    ) -> RetainedWidgetScene {
        let Some(configuration) = self.configuration.as_ref() else {
            return root.clone();
        };
        if !configuration.policy.enabled || !configuration.policy.allow_nonessential_motion {
            self.sources.clear();
            return root.clone();
        }
        let limits = configuration.policy.limits;
        let target_metrics = target.and_then(|target| {
            target
                .metrics_for(
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                    RectF::new(0.0, 0.0, 1.0, 1.0),
                )
                .ok()
        });
        self.target = target_metrics.and_then(|target| {
            target
                .app_frame_attempt
                .map(|attempt| (attempt, target.surface_id))
        });
        let host = target_metrics.map(|target| {
            RectF::new(
                0.0,
                0.0,
                target.target_logical_extent.width,
                target.target_logical_extent.height,
            )
        });
        let font = text.presentation_text_generation();
        let mut work = 0;
        let mut custody_charge = PresentationCharge::default();
        for source in &configuration.sources {
            let captured = certify_source(
                tree,
                cache,
                *source,
                configuration.owner,
                paint_root,
                font.as_ref(),
                scale,
                interpolated,
                appearance,
                limits,
                &mut work,
                self.sources
                    .get(source)
                    .and_then(|value| value.as_ref().ok()),
            );
            let captured = captured.and_then(|value| {
                custody_charge.add(value.charge)?;
                if !custody_charge.within(limits) {
                    return Err(PresentationFailure::RefusedBudget);
                }
                Ok(value)
            });
            self.sources.insert(*source, captured);
        }
        #[cfg(test)]
        {
            self.capture_visits += work;
        }
        let Some(publication) = self.publication.as_ref() else {
            return root.clone();
        };
        if publication.groups.is_empty() {
            return root.clone();
        }
        let mut substitutions = HashMap::new();
        let mut affected = HashSet::new();
        let mut observations = Vec::with_capacity(publication.groups.len());
        let mut result = root.clone();
        let mut frame_charge = custody_charge;
        // Deferred requests/outcomes and the configuration remain live alongside
        // source custody and prepared occurrences throughout this frame.
        if frame_charge
            .add(PresentationCharge {
                cpu_bytes: self.control_bytes,
                ..Default::default()
            })
            .is_err()
        {
            self.expire_frame();
            return root.clone();
        }
        let mut frame_occurrences = 0usize;
        let mut frame_generations: HashSet<_> = self
            .sources
            .values()
            .filter_map(|value| value.as_ref().ok().map(|source| source.generation))
            .collect();
        for group in &publication.groups {
            let prepared = (|| {
                if group.policy_revision != self.revision {
                    return Err(PresentationFailure::RefusedOwnership);
                }
                let host = host.filter(|_| self.target.is_some()).ok_or(
                    PresentationFailure::PendingCapability(
                        PendingPresentationCapability::ExecutionTarget,
                    ),
                )?;
                let prepared = prepare_group(group, &self.sources, font.as_ref(), host, limits)?;
                let mut candidate_charge = frame_charge;
                candidate_charge.add(prepared.charge)?;
                let mut candidate_generations = frame_generations.clone();
                candidate_generations.extend(prepared.retained_generations.iter().copied());
                let candidate_occurrences = frame_occurrences
                    .checked_add(prepared.occurrences.len())
                    .ok_or(PresentationFailure::RefusedBudget)?;
                if !candidate_charge.within(limits)
                    || candidate_charge.cpu_bytes > limits.in_flight_bytes
                    || candidate_occurrences > limits.occurrences
                {
                    return Err(PresentationFailure::RefusedBudget);
                }
                if candidate_generations.len() > limits.generations
                    || candidate_charge
                        .nodes
                        .checked_add(work)
                        .is_none_or(|work| work > limits.capture_nodes_per_frame)
                {
                    return Err(PresentationFailure::RefusedBudget);
                }
                // Build a whole group's candidate before installing any cut.
                let mut cuts: HashMap<PresentationSource, Vec<SceneNode>> = HashMap::new();
                for occurrence in &prepared.occurrences {
                    cuts.entry(occurrence.source.address).or_default().push(
                        SceneNode::RetainedFragment(crate::core::scene::RetainedFragmentNode {
                            children: occurrence.materialize()?,
                            control_subtree_member: None,
                        }),
                    );
                }
                let complete_cuts = cuts
                    .into_iter()
                    .map(|(address, paint)| {
                        let current = self
                            .sources
                            .get(&address)
                            .and_then(|value| value.as_ref().ok())
                            .ok_or(PresentationFailure::RefusedSource)?;
                        Ok((address, paint, current))
                    })
                    .collect::<Result<Vec<_>, PresentationFailure>>()?;
                for (address, paint, current) in complete_cuts {
                    substitutions.insert(
                        fragment_address(&current.paint),
                        Arc::<[SceneNode]>::from(paint),
                    );
                    let mut parent = tree.find_by_id(address.owner());
                    while let Some(node) = parent {
                        if let Some(entry) =
                            tree.node(node).and_then(|node| cache.entries.get(&node.id))
                        {
                            affected.insert(fragment_address(&entry.composed));
                        }
                        parent = tree.parent(node);
                    }
                    #[cfg(debug_assertions)]
                    for key in current.authored_affinity.keys() {
                        result.key_widget_map.remove(key);
                    }
                }
                #[cfg(debug_assertions)]
                for occurrence in &prepared.occurrences {
                    for (key, owner) in &occurrence.source.authored_affinity {
                        result
                            .key_widget_map
                            .insert(occurrence.id.qualify(*key), *owner);
                    }
                }
                frame_charge = candidate_charge;
                frame_generations = candidate_generations;
                frame_occurrences = candidate_occurrences;
                Ok(prepared)
            })();
            observations.push(GroupObservation {
                ticket: group.ticket,
                prepared,
                execution: None,
            });
        }
        result.composed = substitute_fragment(&root.composed, &substitutions, &affected);
        self.pending_execution = self.target;
        self.coordinator.observe(observations);
        result
    }
}

fn fragment_address(fragment: &Arc<[SceneNode]>) -> usize {
    Arc::as_ptr(fragment) as *const SceneNode as usize
}

fn substitute_fragment(
    fragment: &Arc<[SceneNode]>,
    cuts: &HashMap<usize, Arc<[SceneNode]>>,
    affected: &HashSet<usize>,
) -> Arc<[SceneNode]> {
    if let Some(value) = cuts.get(&fragment_address(fragment)) {
        return value.clone();
    }
    if !affected.contains(&fragment_address(fragment)) {
        return fragment.clone();
    }
    fn values(
        nodes: &[SceneNode],
        cuts: &HashMap<usize, Arc<[SceneNode]>>,
        affected: &HashSet<usize>,
    ) -> Vec<SceneNode> {
        nodes
            .iter()
            .map(|node| match node {
                SceneNode::RetainedFragment(value) => {
                    SceneNode::RetainedFragment(crate::core::scene::RetainedFragmentNode {
                        children: substitute_fragment(&value.children, cuts, affected),
                        control_subtree_member: None,
                    })
                }
                SceneNode::Clip(value) => {
                    let mut value = value.clone();
                    value.children = values(&value.children, cuts, affected);
                    SceneNode::Clip(value)
                }
                SceneNode::Scroll(value) => {
                    let mut value = value.clone();
                    value.children = values(&value.children, cuts, affected);
                    SceneNode::Scroll(value)
                }
                _ => node.clone(),
            })
            .collect()
    }
    values(fragment, cuts, affected).into()
}

fn certify_source(
    tree: &WidgetTree,
    cache: &RetainedSceneCache,
    address: PresentationSource,
    owner: TreeWidgetId,
    paint_root: NodeId,
    font: Option<&TextGenerationLease>,
    scale: f32,
    interpolated: &InterpolatedVisualRects,
    appearance: Option<&FrameworkAppearanceSnapshot>,
    limits: PresentationLimits,
    work: &mut usize,
    previous: Option<&Arc<CertifiedSource>>,
) -> Result<Arc<CertifiedSource>, PresentationFailure> {
    let node_id = tree
        .find_by_id(address.owner())
        .ok_or(PresentationFailure::RefusedSource)?;
    let owner_node = tree
        .find_by_id(owner)
        .ok_or(PresentationFailure::RefusedOwnership)?;
    if !tree.is_descendant_or_same(node_id, owner_node) {
        return Err(PresentationFailure::RefusedOwnership);
    }
    if !tree.is_descendant_or_same(node_id, paint_root) {
        return Err(PresentationFailure::RefusedCoverage);
    }
    let node = tree
        .node(node_id)
        .ok_or(PresentationFailure::RefusedSource)?;
    let entry = cache
        .entries
        .get(&node.id)
        .ok_or(PresentationFailure::RefusedSource)?;
    let paint = match address {
        PresentationSource::Widget(_) => &entry.local,
        _ => &entry.composed,
    };
    let bounds = tree
        .get_absolute_paint_rect(node_id)
        .ok_or(PresentationFailure::RefusedCoverage)?;
    if bounds.width <= 0.0 && bounds.height <= 0.0 {
        return Err(PresentationFailure::RefusedSource);
    }
    if let Some(previous) = previous {
        if Arc::ptr_eq(&previous.paint, paint)
            && previous.bounds == bounds
            && previous.structure_generation == tree.structure_generation()
            && previous.scale_bits == scale.to_bits()
            && match (&previous.font, font) {
                (Some(a), Some(b)) => a.same_generation(b),
                (None, _) => true,
                _ => false,
            }
        {
            for owner in previous.members.iter().chain(previous.host_path.iter()) {
                if interpolated.contains_key(owner) {
                    return Err(PresentationFailure::RefusedOwnership);
                }
                let id = tree
                    .find_by_id(*owner)
                    .ok_or(PresentationFailure::RefusedSource)?;
                reject_conflicting_scope(tree, id)?;
            }
            for owner in previous.members.iter() {
                let node = tree
                    .node(
                        tree.find_by_id(*owner)
                            .ok_or(PresentationFailure::RefusedSource)?,
                    )
                    .ok_or(PresentationFailure::RefusedSource)?;
                certify_producer(node.widget.as_ref(), *owner, appearance)?;
            }
            return Ok(previous.clone());
        }
    }
    let mut host_path = Vec::new();
    let mut parent = tree.parent(node_id);
    while let Some(id) = parent {
        if host_path.len() >= limits.depth {
            return Err(PresentationFailure::RefusedBudget);
        }
        let node = tree.node(id).ok_or(PresentationFailure::RefusedCoverage)?;
        reject_conflicting_scope(tree, id)?;
        if interpolated.contains_key(&node.id) {
            return Err(PresentationFailure::RefusedOwnership);
        }
        host_path.push(node.id);
        parent = tree.parent(id);
    }
    let mut members = Vec::new();
    let mut pending = vec![(node_id, 0)];
    while let Some((id, depth)) = pending.pop() {
        if tree
            .get_absolute_paint_rect(id)
            .is_some_and(|rect| rect.width <= 0.0 && rect.height <= 0.0)
        {
            continue;
        }
        *work += 1;
        if *work > limits.capture_nodes_per_frame
            || depth > limits.depth
            || members.len() >= limits.source_widgets
        {
            return Err(PresentationFailure::RefusedBudget);
        }
        let node = tree.node(id).ok_or(PresentationFailure::RefusedSource)?;
        reject_conflicting_scope(tree, id)?;
        if interpolated.contains_key(&node.id) {
            return Err(PresentationFailure::RefusedOwnership);
        }
        let entry = cache
            .entries
            .get(&node.id)
            .ok_or(PresentationFailure::RefusedSource)?;
        if !entry.overlay.is_empty() || !entry.own_overlay_hit_regions.is_empty() {
            return Err(PresentationFailure::PendingCapability(
                PendingPresentationCapability::ScrollOrOverlay,
            ));
        }
        certify_producer(node.widget.as_ref(), node.id, appearance)?;
        members.push(node.id);
        if matches!(address, PresentationSource::Widget(_)) && node.first_child.is_some() {
            return Err(PresentationFailure::RefusedCoverage);
        }
        if node.overflow == OverflowMode::Clip {
            return Err(PresentationFailure::PendingCapability(
                PendingPresentationCapability::ScrollOrOverlay,
            ));
        }
        let mut child = node.first_child;
        let start = pending.len();
        while let Some(id) = child {
            if pending.len() + members.len() >= limits.source_widgets {
                return Err(PresentationFailure::RefusedBudget);
            }
            pending.push((id, depth + 1));
            child = tree
                .node(id)
                .ok_or(PresentationFailure::RefusedSource)?
                .next_sibling;
        }
        pending[start..].reverse();
    }
    let node = tree
        .node(node_id)
        .ok_or(PresentationFailure::RefusedSource)?;
    let entry = cache
        .entries
        .get(&node.id)
        .ok_or(PresentationFailure::RefusedSource)?;
    let paint = match address {
        PresentationSource::Widget(_) => entry.local.clone(),
        PresentationSource::Subtree(_) => entry.composed.clone(),
        PresentationSource::Member { .. } => {
            return Err(PresentationFailure::PendingCapability(
                PendingPresentationCapability::MemberSource,
            ));
        }
    };
    let bounds = tree
        .get_absolute_paint_rect(node_id)
        .ok_or(PresentationFailure::RefusedCoverage)?;
    valid_rect(bounds)?;
    if let Some(previous) = previous {
        if Arc::ptr_eq(&previous.paint, &paint)
            && previous.bounds == bounds
            && previous.host_path.as_ref() == host_path
            && previous.members.as_ref() == members
            && previous.scale_bits == scale.to_bits()
            && match (&previous.font, font) {
                (Some(a), Some(b)) => a.same_generation(b),
                (None, _) => true,
                _ => false,
            }
        {
            return Ok(previous.clone());
        }
    }
    let mut capture_limits = limits;
    capture_limits.nodes = limits
        .nodes
        .min(limits.capture_nodes_per_frame.saturating_sub(*work));
    let mut charge = admit_paint_values(&paint, font, capture_limits)?;
    *work += charge.nodes;
    charge.add(PresentationCharge {
        cpu_bytes: std::mem::size_of::<CertifiedSource>()
            + (members.len() + host_path.len()) * std::mem::size_of::<TreeWidgetId>(),
        edges: members.len() + host_path.len(),
        ..Default::default()
    })?;
    #[cfg(debug_assertions)]
    let authored_affinity = match address {
        PresentationSource::Widget(_) => entry.own_key_widget_map.clone(),
        _ => entry.key_widget_map.clone(),
    };
    #[cfg(debug_assertions)]
    charge.add(PresentationCharge {
        cpu_bytes: authored_affinity.capacity() * (std::mem::size_of::<SceneNodeKey>() + 16),
        ..Default::default()
    })?;
    if !charge.within(limits) {
        return Err(PresentationFailure::RefusedBudget);
    }
    Ok(Arc::new(CertifiedSource {
        address,
        generation: SourceGenerationId(next_value_id()?),
        bounds,
        paint,
        members: members.into(),
        host_path: host_path.into(),
        // Admission charges font backing only when the paint values consume text.
        font: font.filter(|_| charge.font_bytes != 0).cloned(),
        scale_bits: scale.to_bits(),
        structure_generation: tree.structure_generation(),
        charge,
        #[cfg(debug_assertions)]
        authored_affinity,
    }))
}

fn reject_conflicting_scope(tree: &WidgetTree, id: NodeId) -> Result<(), PresentationFailure> {
    let node = tree.node(id).ok_or(PresentationFailure::RefusedCoverage)?;
    if node.widget.clip_bounds_rounded().is_some() {
        return Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::SoftMask,
        ));
    }
    if node.layout_scroll_offset.is_some()
        || node.widget.scroll_offset().is_some()
        || tree.get_input_projection(id).is_some()
        || node.widget.has_pending_anchor_resolution()
    {
        return Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::ScrollOrOverlay,
        ));
    }
    Ok(())
}

fn certify_producer(
    widget: &dyn Widget,
    owner: TreeWidgetId,
    appearance: Option<&FrameworkAppearanceSnapshot>,
) -> Result<(), PresentationFailure> {
    use crate::widgets::{button::Button, flex::Flex, label::Label};
    if let Some(facts) = appearance.map(|value| value.for_owner(owner)) {
        if facts.focused
            || facts.keyboard_captured
            || facts.pointer_captured
            || facts.scroll_captured
        {
            return Err(PresentationFailure::RefusedCue);
        }
    }
    if let Some(button) = widget.as_any().downcast_ref::<Button>() {
        let resolved = button
            .resolved_appearance()
            .map_err(|_| PresentationFailure::RefusedSource)?;
        let input = resolved.request().input;
        let facts = input.button();
        if appearance.is_none()
            || !facts.enabled
            || facts.hovered
            || facts.pressed
            || input.selection().is_some()
            || !resolved.request().selector.qualifiers().is_empty()
        {
            return Err(PresentationFailure::RefusedCue);
        }
        return Ok(());
    }
    if let Some(label) = widget.as_any().downcast_ref::<Label>() {
        return if label.capture_eligible() {
            Err(PresentationFailure::RefusedCue)
        } else {
            Ok(())
        };
    }
    if widget.as_any().is::<Flex>() {
        return Ok(());
    }
    Err(PresentationFailure::PendingCapability(
        PendingPresentationCapability::SourceProducer,
    ))
}

#[cfg(test)]
mod tests;
