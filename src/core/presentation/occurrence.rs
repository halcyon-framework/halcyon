use std::sync::Arc;

use crate::core::math::{Point, RectF};
use crate::core::scene::{ClipNode, RetainedFragmentNode, SceneNode};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::TextGenerationLease;
use crate::utils::id::TreeWidgetId;

use super::{PresentationLimits, PresentationSource};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PendingPresentationCapability {
    Affine,
    IsolatedOpacity,
    SoftMask,
    MemberSource,
    ResourceContentLease,
    SourceProducer,
    ScrollOrOverlay,
    TextGeneration,
    ExecutionTarget,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PresentationFailure {
    PendingCapability(PendingPresentationCapability),
    RefusedOwnership,
    RefusedSource,
    RefusedCoverage,
    RefusedCue,
    RefusedBudget,
    UnavailableResource,
    ExpiredFirstPublication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct OccurrenceId(u64);

impl OccurrenceId {
    pub(crate) fn new(value: u64) -> Self {
        assert_ne!(value, 0);
        Self(value)
    }
    #[cfg(debug_assertions)]
    pub(crate) fn qualify(self, key: SceneNodeKey) -> SceneNodeKey {
        key.for_occurrence(self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SourceRole {
    Current,
    Historical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct SourceGenerationId(pub(crate) u64);

/// Produced by the retained traversal, not by request authors. Widget membership
/// is provenance only; the generation holds no widget or semantic sidecar.
#[derive(Debug, Clone)]
pub(crate) struct CertifiedSource {
    pub address: PresentationSource,
    pub generation: SourceGenerationId,
    pub bounds: RectF,
    pub paint: Arc<[SceneNode]>,
    pub members: Arc<[TreeWidgetId]>,
    pub host_path: Arc<[TreeWidgetId]>,
    pub font: Option<TextGenerationLease>,
    pub scale_bits: u32,
    pub structure_generation: u64,
    pub charge: PresentationCharge,
    #[cfg(debug_assertions)]
    pub authored_affinity: std::collections::HashMap<SceneNodeKey, u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct PresentationCharge {
    pub nodes: usize,
    pub cpu_bytes: usize,
    pub text_bytes: usize,
    pub font_bytes: usize,
    pub depth: usize,
    pub edges: usize,
}

impl PresentationCharge {
    pub(crate) fn add(&mut self, other: Self) -> Result<(), PresentationFailure> {
        self.nodes = self
            .nodes
            .checked_add(other.nodes)
            .ok_or(PresentationFailure::RefusedBudget)?;
        self.cpu_bytes = self
            .cpu_bytes
            .checked_add(other.cpu_bytes)
            .ok_or(PresentationFailure::RefusedBudget)?;
        self.text_bytes = self
            .text_bytes
            .checked_add(other.text_bytes)
            .ok_or(PresentationFailure::RefusedBudget)?;
        self.font_bytes = self
            .font_bytes
            .checked_add(other.font_bytes)
            .ok_or(PresentationFailure::RefusedBudget)?;
        self.edges = self
            .edges
            .checked_add(other.edges)
            .ok_or(PresentationFailure::RefusedBudget)?;
        self.depth = self.depth.max(other.depth);
        Ok(())
    }

    pub(crate) fn within(&self, limits: PresentationLimits) -> bool {
        self.nodes <= limits.nodes
            && self.cpu_bytes <= limits.cpu_bytes
            && self.text_bytes <= limits.text_bytes
            && self.font_bytes <= limits.font_bytes
            && self.depth <= limits.depth
            && self.edges <= limits.dependency_edges
    }
}

pub(crate) fn admit_paint_values(
    nodes: &[SceneNode],
    font: Option<&TextGenerationLease>,
    limits: PresentationLimits,
) -> Result<PresentationCharge, PresentationFailure> {
    fn visit(
        nodes: &[SceneNode],
        depth: usize,
        charge: &mut PresentationCharge,
        has_text: &mut bool,
        limits: PresentationLimits,
    ) -> Result<(), PresentationFailure> {
        if depth > limits.depth {
            return Err(PresentationFailure::RefusedBudget);
        }
        for node in nodes {
            charge.add(PresentationCharge {
                nodes: 1,
                cpu_bytes: std::mem::size_of::<SceneNode>(),
                depth,
                edges: 1,
                ..Default::default()
            })?;
            if !charge.within(limits) {
                return Err(PresentationFailure::RefusedBudget);
            }
            match node {
                SceneNode::RetainedFragment(fragment) => {
                    charge.add(PresentationCharge {
                        cpu_bytes: 2 * std::mem::size_of::<usize>(),
                        ..Default::default()
                    })?;
                    visit(&fragment.children, depth + 1, charge, has_text, limits)?;
                }
                SceneNode::Clip(clip) => {
                    if clip.rounded.is_some() {
                        return Err(PresentationFailure::PendingCapability(
                            PendingPresentationCapability::SoftMask,
                        ));
                    }
                    valid_rect(clip.rect)?;
                    charge.add(PresentationCharge {
                        cpu_bytes: (clip.children.capacity() - clip.children.len())
                            * std::mem::size_of::<SceneNode>(),
                        ..Default::default()
                    })?;
                    visit(&clip.children, depth + 1, charge, has_text, limits)?;
                }
                SceneNode::Rect(rect) => {
                    valid_rect(rect.rect)?;
                    valid_color(rect.color)?;
                    valid_z(rect.z_index)?;
                }
                SceneNode::BorderRect(rect) => {
                    valid_rect(rect.rect)?;
                    valid_color(rect.fill_color)?;
                    valid_color(rect.border_color)?;
                    valid_z(rect.z_index)?;
                    if !rect.radius.is_finite()
                        || rect.radius < 0.0
                        || !rect.border_width.is_finite()
                        || rect.border_width < 0.0
                    {
                        return Err(PresentationFailure::RefusedSource);
                    }
                }
                SceneNode::RoundedRect(rect) => {
                    valid_rect(rect.rect)?;
                    valid_color(rect.color)?;
                    valid_z(rect.z_index)?;
                    if !rect.radius.is_finite() || rect.radius < 0.0 {
                        return Err(PresentationFailure::RefusedSource);
                    }
                }
                SceneNode::Text(text) => {
                    valid_color(text.color)?;
                    valid_z(text.z_index)?;
                    if !text.position.x.is_finite()
                        || !text.position.y.is_finite()
                        || !text.font_size.is_finite()
                        || text.font_size <= 0.0
                    {
                        return Err(PresentationFailure::RefusedSource);
                    }
                    // The supported producer profile has no selection/caret cues.
                    if !text.backgrounds.is_empty() {
                        return Err(PresentationFailure::RefusedCue);
                    }
                    *has_text = true;
                    charge.add(PresentationCharge {
                        cpu_bytes: text.text.capacity(),
                        text_bytes: text.text.len(),
                        ..Default::default()
                    })?;
                }
                SceneNode::Image(_) => {
                    return Err(PresentationFailure::PendingCapability(
                        PendingPresentationCapability::ResourceContentLease,
                    ));
                }
                SceneNode::Scroll(_) => {
                    return Err(PresentationFailure::PendingCapability(
                        PendingPresentationCapability::ScrollOrOverlay,
                    ));
                }
                SceneNode::Layer(_) => {
                    return Err(PresentationFailure::PendingCapability(
                        PendingPresentationCapability::IsolatedOpacity,
                    ));
                }
                _ => {
                    return Err(PresentationFailure::PendingCapability(
                        PendingPresentationCapability::SourceProducer,
                    ));
                }
            }
        }
        Ok(())
    }
    let mut charge = PresentationCharge::default();
    let mut has_text = false;
    visit(nodes, 1, &mut charge, &mut has_text, limits)?;
    if has_text {
        charge.font_bytes = font
            .ok_or(PresentationFailure::PendingCapability(
                PendingPresentationCapability::TextGeneration,
            ))?
            .retained_bytes();
        charge.cpu_bytes = charge
            .cpu_bytes
            .checked_add(charge.font_bytes)
            .ok_or(PresentationFailure::RefusedBudget)?;
    }
    if !charge.within(limits) {
        return Err(PresentationFailure::RefusedBudget);
    }
    Ok(charge)
}

pub(crate) fn valid_rect(rect: RectF) -> Result<(), PresentationFailure> {
    if [
        rect.x,
        rect.y,
        rect.width,
        rect.height,
        rect.x + rect.width,
        rect.y + rect.height,
    ]
    .iter()
    .all(|v| v.is_finite())
        && rect.width >= 0.0
        && rect.height >= 0.0
    {
        Ok(())
    } else {
        Err(PresentationFailure::RefusedCoverage)
    }
}

fn valid_color(color: crate::core::scene::Color) -> Result<(), PresentationFailure> {
    if [color.r, color.g, color.b, color.a]
        .iter()
        .all(|v| v.is_finite())
        && (0.0..=1.0).contains(&color.a)
    {
        Ok(())
    } else {
        Err(PresentationFailure::RefusedSource)
    }
}

fn valid_z(z: u32) -> Result<(), PresentationFailure> {
    if z >= crate::core::scene::LAYER_OVERLAY {
        Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::ScrollOrOverlay,
        ))
    } else {
        Ok(())
    }
}

fn translate_rect(rect: RectF, offset: Point) -> Result<RectF, PresentationFailure> {
    let result = RectF::new(
        rect.x + offset.x,
        rect.y + offset.y,
        rect.width,
        rect.height,
    );
    if [result.x, result.y, result.width, result.height]
        .iter()
        .all(|v| v.is_finite())
    {
        Ok(result)
    } else {
        Err(PresentationFailure::RefusedCoverage)
    }
}

/// Called only after traversal has certified the closed source cut. External
/// barriers remain outside this value tree, at their original host scopes.
pub(crate) fn project_paint_values(
    source: &[SceneNode],
    occurrence: OccurrenceId,
    role: SourceRole,
    translation: Point,
) -> Result<Arc<[SceneNode]>, PresentationFailure> {
    if !translation.x.is_finite() || !translation.y.is_finite() {
        return Err(PresentationFailure::RefusedCoverage);
    }
    project_nodes(source, occurrence, role, translation, &mut 0)
}

fn project_nodes(
    source: &[SceneNode],
    occurrence: OccurrenceId,
    role: SourceRole,
    translation: Point,
    ordinal: &mut u64,
) -> Result<Arc<[SceneNode]>, PresentationFailure> {
    let mut projected = Vec::with_capacity(source.len());
    for node in source {
        let key = node.key().map_or_else(
            || SceneNodeKey::anonymous_occurrence(occurrence.0, *ordinal),
            |key| key.for_occurrence(occurrence.0),
        );
        *ordinal = ordinal
            .checked_add(1)
            .ok_or(PresentationFailure::RefusedBudget)?;
        let value = match node {
            SceneNode::RetainedFragment(fragment) => {
                SceneNode::RetainedFragment(RetainedFragmentNode {
                    children: project_nodes(
                        &fragment.children,
                        occurrence,
                        role,
                        translation,
                        ordinal,
                    )?,
                    control_subtree_member: None,
                })
            }
            SceneNode::Clip(clip) => {
                if clip.rounded.is_some() {
                    return Err(PresentationFailure::PendingCapability(
                        PendingPresentationCapability::SoftMask,
                    ));
                }
                let mut clip = clip.clone();
                if role == SourceRole::Historical {
                    clip.rect = translate_rect(clip.rect, translation)?;
                }
                clip.children =
                    project_nodes(&clip.children, occurrence, role, translation, ordinal)?.to_vec();
                clip.key = Some(key);
                SceneNode::Clip(clip)
            }
            SceneNode::Rect(rect) => {
                let mut rect = rect.clone();
                rect.rect = translate_rect(rect.rect, translation)?;
                rect.key = Some(key);
                SceneNode::Rect(rect)
            }
            SceneNode::BorderRect(rect) => {
                let mut rect = rect.clone();
                rect.rect = translate_rect(rect.rect, translation)?;
                rect.key = Some(key);
                SceneNode::BorderRect(rect)
            }
            SceneNode::RoundedRect(rect) => {
                let mut rect = rect.clone();
                rect.rect = translate_rect(rect.rect, translation)?;
                rect.key = Some(key);
                SceneNode::RoundedRect(rect)
            }
            SceneNode::Text(text) => {
                let mut text = text.clone();
                text.position.x += translation.x;
                text.position.y += translation.y;
                if !text.position.x.is_finite() || !text.position.y.is_finite() {
                    return Err(PresentationFailure::RefusedCoverage);
                }
                text.key = Some(key);
                SceneNode::Text(text)
            }
            SceneNode::Image(_) => {
                return Err(PresentationFailure::PendingCapability(
                    PendingPresentationCapability::ResourceContentLease,
                ));
            }
            SceneNode::Scroll(_) => {
                return Err(PresentationFailure::PendingCapability(
                    PendingPresentationCapability::ScrollOrOverlay,
                ));
            }
            SceneNode::Layer(_) => {
                return Err(PresentationFailure::PendingCapability(
                    PendingPresentationCapability::IsolatedOpacity,
                ));
            }
            _ => {
                return Err(PresentationFailure::PendingCapability(
                    PendingPresentationCapability::SourceProducer,
                ));
            }
        };
        projected.push(value);
    }
    Ok(projected.into())
}

/// A binary scissor has once-per-group coverage equivalent to the same binary
/// restriction on every contribution. This admits no soft/rounded mask algebra.
pub(crate) fn apply_presentation_mask(source: Arc<[SceneNode]>, mask: RectF) -> Arc<[SceneNode]> {
    Arc::from([SceneNode::Clip(ClipNode {
        rect: mask,
        rounded: None,
        children: vec![SceneNode::RetainedFragment(RetainedFragmentNode {
            children: source,
            control_subtree_member: None,
        })],
        z_index: 0,
        key: None,
    })])
}

pub(crate) fn paint_values_equal(a: &[SceneNode], b: &[SceneNode]) -> bool {
    a.len() == b.len()
        && a.iter().zip(b).all(|(a, b)| match (a, b) {
            (SceneNode::RetainedFragment(a), SceneNode::RetainedFragment(b)) => {
                paint_values_equal(&a.children, &b.children)
            }
            (SceneNode::Clip(a), SceneNode::Clip(b)) => {
                a.rect == b.rect
                    && a.rounded == b.rounded
                    && a.key == b.key
                    && a.z_index == b.z_index
                    && paint_values_equal(&a.children, &b.children)
            }
            (SceneNode::Rect(a), SceneNode::Rect(b)) => {
                a.rect == b.rect && a.color == b.color && a.key == b.key && a.z_index == b.z_index
            }
            (SceneNode::RoundedRect(a), SceneNode::RoundedRect(b)) => {
                a.rect == b.rect
                    && a.color == b.color
                    && a.radius == b.radius
                    && a.key == b.key
                    && a.z_index == b.z_index
            }
            (SceneNode::BorderRect(a), SceneNode::BorderRect(b)) => {
                a.rect == b.rect
                    && a.fill_color == b.fill_color
                    && a.border_color == b.border_color
                    && a.border_width == b.border_width
                    && a.radius == b.radius
                    && a.key == b.key
                    && a.z_index == b.z_index
            }
            (SceneNode::Text(a), SceneNode::Text(b)) => {
                a.text == b.text
                    && a.font_size == b.font_size
                    && a.color == b.color
                    && a.position == b.position
                    && a.key == b.key
                    && a.z_index == b.z_index
                    && a.backgrounds.is_empty()
                    && b.backgrounds.is_empty()
            }
            _ => false,
        })
}
