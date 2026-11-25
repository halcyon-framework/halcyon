use super::*;
use crate::core::math::{Point, RectF};
use crate::core::scene::{ClipNode, Color, RectNode, SceneNode};
use crate::core::scene_identity::SceneNodeKey;
use std::sync::Arc;

pub(crate) fn limits() -> PresentationLimits {
    PresentationLimits {
        groups: 2,
        queued_requests: 8,
        sources: 16,
        source_widgets: 64,
        occurrences: 64,
        generations: 64,
        nodes: 4096,
        depth: 32,
        dependency_edges: 8192,
        cpu_bytes: 512 * 1024 * 1024,
        text_bytes: 65536,
        font_bytes: 512 * 1024 * 1024,
        resource_bytes: 0,
        in_flight_bytes: 512 * 1024 * 1024,
        capture_nodes_per_frame: 4096,
        max_duration_secs: 2.0,
        max_historical_age_ms: 4000,
    }
}

#[test]
fn transient_presentation_request_is_idempotent_and_conflicting_cause_is_refused() {
    let source = PresentationSource::Widget(crate::utils::id::TreeWidgetId(1));
    let mut inbox = PresentationInbox::new(PresentationConfiguration {
        owner: source.owner(),
        sources: vec![source],
        policy: PresentationMotionPolicy {
            enabled: true,
            allow_nonessential_motion: true,
            limits: limits(),
        },
    })
    .unwrap();
    let request = PresentationRequest {
        cause: 1,
        composition: PresentationComposition::Projections(vec![PresentationProjection {
            source,
            transform: PresentationTransform::Translation {
                from: Point::new(0.0, 20.0),
                to: Point::zero(),
            },
            mask: PresentationMask::None,
            isolated_opacity: None,
        }]),
        duration_secs: 0.3,
        easing: crate::core::animation::Easing::Linear,
        replaces: None,
        settlement: PresentationSettlement::Canonical,
    };
    let first = inbox.request(request.clone()).unwrap();
    assert_eq!(inbox.request(request.clone()).unwrap(), first);
    assert_eq!(inbox.requests.len(), 1);
    let mut conflicting = request.clone();
    conflicting.duration_secs = 0.6;
    assert_eq!(
        inbox.request(conflicting),
        Err(PresentationFailure::RefusedSource)
    );

    let mut other = PresentationInbox::new(inbox.configuration.clone()).unwrap();
    let other_ticket = other.request(request).unwrap();
    assert_ne!(
        first, other_ticket,
        "request lifetimes must not alias across owners"
    );
}

#[test]
fn transient_presentation_affine_request_is_pending_without_translation_substitution() {
    let source = PresentationSource::Widget(crate::utils::id::TreeWidgetId(1));
    let composition = PresentationComposition::Projections(vec![PresentationProjection {
        source,
        transform: PresentationTransform::Affine {
            from: [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            to: [0.0, 1.0, -1.0, 0.0, 0.0, 0.0],
        },
        mask: PresentationMask::None,
        isolated_opacity: None,
    }]);
    assert_eq!(
        composition.preliminary_admission(),
        Err(PresentationFailure::PendingCapability(
            PendingPresentationCapability::Affine
        ))
    );
}

fn source() -> Arc<[SceneNode]> {
    Arc::from([SceneNode::Clip(ClipNode {
        rect: RectF::new(10.0, 20.0, 80.0, 30.0),
        rounded: None,
        children: vec![SceneNode::Rect(RectNode {
            rect: RectF::new(12.0, 22.0, 70.0, 24.0),
            color: Color::WHITE,
            z_index: 0,
            key: Some(SceneNodeKey::new(42)),
        })],
        z_index: 0,
        key: None,
    })])
}

fn clip_and_rect(nodes: &[SceneNode]) -> (&ClipNode, &RectNode) {
    let SceneNode::Clip(clip) = &nodes[0] else {
        panic!("source clip lost")
    };
    let SceneNode::Rect(rect) = &clip.children[0] else {
        panic!("complete leaf lost")
    };
    (clip, rect)
}

#[test]
fn transient_presentation_current_clip_stays_canonical_while_complete_content_moves() {
    let source = source();
    let values = project_paint_values(
        &source,
        OccurrenceId::new(1),
        SourceRole::Current,
        Point::new(7.0, 11.0),
    )
    .unwrap();
    let (clip, rect) = clip_and_rect(&values);
    assert_eq!(clip.rect, RectF::new(10.0, 20.0, 80.0, 30.0));
    assert_eq!(rect.rect, RectF::new(19.0, 33.0, 70.0, 24.0));
    assert_eq!(
        clip_and_rect(&source).1.rect,
        RectF::new(12.0, 22.0, 70.0, 24.0)
    );
}

#[test]
fn transient_presentation_only_historical_internal_coverage_travels() {
    let values = project_paint_values(
        &source(),
        OccurrenceId::new(2),
        SourceRole::Historical,
        Point::new(7.0, 11.0),
    )
    .unwrap();
    let (clip, rect) = clip_and_rect(&values);
    assert_eq!(clip.rect, RectF::new(17.0, 31.0, 80.0, 30.0));
    assert_eq!(rect.rect, RectF::new(19.0, 33.0, 70.0, 24.0));
}

#[test]
fn transient_presentation_occurrence_identity_is_stable_and_cannot_alias_authored_identity() {
    let source = source();
    let first = project_paint_values(
        &source,
        OccurrenceId::new(1),
        SourceRole::Current,
        Point::zero(),
    )
    .unwrap();
    let moved = project_paint_values(
        &source,
        OccurrenceId::new(1),
        SourceRole::Current,
        Point::new(1.0, 2.0),
    )
    .unwrap();
    let historical = project_paint_values(
        &source,
        OccurrenceId::new(2),
        SourceRole::Historical,
        Point::zero(),
    )
    .unwrap();
    assert_eq!(clip_and_rect(&first).1.key, clip_and_rect(&moved).1.key);
    assert_ne!(
        clip_and_rect(&first).1.key,
        clip_and_rect(&historical).1.key
    );
    assert_ne!(clip_and_rect(&first).1.key, Some(SceneNodeKey::new(42)));
    assert_ne!(clip_and_rect(&first).0.key, clip_and_rect(&first).1.key);
}

#[test]
fn transient_presentation_mask_is_additional_and_does_not_replace_source_coverage() {
    let values = project_paint_values(
        &source(),
        OccurrenceId::new(1),
        SourceRole::Current,
        Point::new(0.0, 10.0),
    )
    .unwrap();
    let mask = RectF::new(0.0, 0.0, 200.0, 43.0);
    let masked = apply_presentation_mask(values, mask);
    let SceneNode::Clip(outer) = &masked[0] else {
        panic!("presentation mask missing")
    };
    assert_eq!(outer.rect, mask);
    let SceneNode::RetainedFragment(values) = &outer.children[0] else {
        panic!("mask scope lost")
    };
    assert_eq!(
        clip_and_rect(&values.children).0.rect,
        RectF::new(10.0, 20.0, 80.0, 30.0)
    );
}
