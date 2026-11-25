use halcyon::core::animation::{
    Animation, AnimationContext, AnimationDescriptor, AnimationSample, AnimationStartError, Easing,
    Interpolatable, Playback, RepeatCount, Timeline, TimelineAdvanceError,
};
use halcyon::core::math::Point;
use halcyon::core::paint::Color;
use halcyon::core::widget::UpdateContext;

fn descriptor(playback: Playback) -> AnimationDescriptor<f32> {
    Animation::new(0.0, 10.0, 1.0, Easing::Linear).with_playback(playback)
}

#[test]
fn timeline_contract_completion_and_cancellation_remain_observable_for_their_frame() {
    let mut timeline = Timeline::with_capacity(4);
    timeline.update(10.0).unwrap();

    let completed = timeline
        .start(Animation::new(0.0f32, 10.0, 1.0, Easing::Linear))
        .unwrap();
    assert_eq!(
        timeline.sample::<f32>(completed),
        AnimationSample::Active(0.0)
    );
    assert_eq!(
        timeline.sample::<f64>(completed),
        AnimationSample::TypeMismatch
    );

    timeline.update(10.5).unwrap();
    assert_eq!(
        timeline.sample::<f32>(completed),
        AnimationSample::Active(5.0)
    );

    timeline.update(11.0).unwrap();
    assert_eq!(
        timeline.sample::<f32>(completed),
        AnimationSample::Completed(10.0)
    );
    assert_eq!(timeline.get(completed), Some(10.0f32));
    assert!(!timeline.is_active(completed));
    assert!(!timeline.has_active_animations());
    assert_eq!(timeline.retained_entry_count(), 1);

    timeline.update(11.25).unwrap();
    assert_eq!(timeline.sample::<f32>(completed), AnimationSample::Missing);

    let cancelled = timeline
        .start(Animation::new(3.0f32, 9.0, 1.0, Easing::Linear))
        .unwrap();
    timeline.cancel(cancelled);
    timeline.cancel(cancelled);
    assert_eq!(
        timeline.sample::<f32>(cancelled),
        AnimationSample::Cancelled
    );
    assert!(!timeline.has_active_animations());

    timeline.update(11.5).unwrap();
    assert_eq!(timeline.sample::<f32>(cancelled), AnimationSample::Missing);
}

#[test]
fn timeline_contract_loop_and_alternate_use_absolute_time_and_exact_boundaries() {
    let mut looped = Timeline::with_capacity(2);
    looped.update(0.0).unwrap();
    let looped_handle = looped
        .start_descriptor(descriptor(Playback::Loop(RepeatCount::Finite(2))))
        .unwrap();

    looped.update(0.5).unwrap();
    assert_eq!(
        looped.sample::<f32>(looped_handle),
        AnimationSample::Active(5.0)
    );
    looped.update(1.0).unwrap();
    assert_eq!(
        looped.sample::<f32>(looped_handle),
        AnimationSample::Active(0.0)
    );
    looped.update(1.5).unwrap();
    assert_eq!(
        looped.sample::<f32>(looped_handle),
        AnimationSample::Active(5.0)
    );
    looped.update(2.0).unwrap();
    assert_eq!(
        looped.sample::<f32>(looped_handle),
        AnimationSample::Completed(10.0)
    );

    let mut alternating = Timeline::with_capacity(2);
    alternating.update(0.0).unwrap();
    let odd = alternating
        .start_descriptor(descriptor(Playback::Alternate(RepeatCount::Finite(3))))
        .unwrap();

    alternating.update(1.0).unwrap();
    assert_eq!(
        alternating.sample::<f32>(odd),
        AnimationSample::Active(10.0)
    );
    alternating.update(1.5).unwrap();
    assert_eq!(alternating.sample::<f32>(odd), AnimationSample::Active(5.0));
    alternating.update(2.0).unwrap();
    assert_eq!(alternating.sample::<f32>(odd), AnimationSample::Active(0.0));
    alternating.update(3.0).unwrap();
    assert_eq!(
        alternating.sample::<f32>(odd),
        AnimationSample::Completed(10.0)
    );

    alternating.update(3.25).unwrap();
    let even = alternating
        .start_descriptor(descriptor(Playback::Alternate(RepeatCount::Finite(2))))
        .unwrap();
    alternating.update(5.25).unwrap();
    assert_eq!(
        alternating.sample::<f32>(even),
        AnimationSample::Completed(0.0)
    );
}

#[test]
fn timeline_contract_infinite_playback_is_explicit_and_rejects_unrepresentable_elapsed_time() {
    let mut timeline = Timeline::with_capacity(2);
    timeline.update(0.0).unwrap();
    let looped = timeline
        .start_descriptor(descriptor(Playback::Loop(RepeatCount::Infinite)))
        .unwrap();

    timeline.update(3.25).unwrap();
    assert_eq!(timeline.sample::<f32>(looped), AnimationSample::Active(2.5));
    assert!(timeline.has_active_animations());

    assert_eq!(
        timeline.update(9_007_199_254_742_000.0),
        Err(TimelineAdvanceError::UnrepresentableElapsed)
    );
    assert_eq!(timeline.sample::<f32>(looped), AnimationSample::Active(2.5));

    timeline.cancel(looped);
    assert_eq!(timeline.sample::<f32>(looped), AnimationSample::Cancelled);
    assert!(!timeline.has_active_animations());
}

#[derive(Clone)]
struct Unsupported(f32);

impl Interpolatable for Unsupported {
    fn lerp(from: &Self, to: &Self, progress: f32) -> Self {
        Self(from.0 + (to.0 - from.0) * progress)
    }
}

#[test]
fn timeline_contract_admission_rejects_invalid_descriptors_unsupported_payloads_and_exhaustion() {
    let mut timeline = Timeline::with_capacity(1);

    assert_eq!(
        timeline.start(Animation {
            from: 0.0f32,
            to: 1.0,
            duration: 0.0,
            easing: Easing::Linear,
        }),
        Err(AnimationStartError::InvalidDuration)
    );
    for duration in [-1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            timeline.start(Animation {
                from: 0.0f32,
                to: 1.0,
                duration,
                easing: Easing::Linear,
            }),
            Err(AnimationStartError::InvalidDuration)
        );
    }
    assert_eq!(
        timeline.start(Animation::new(
            0.0f32,
            1.0,
            1.0,
            Easing::CubicBezier {
                p1: (f32::NAN, 0.0),
                p2: (1.0, 1.0),
            },
        )),
        Err(AnimationStartError::InvalidEasing)
    );
    assert_eq!(
        timeline.start_descriptor(descriptor(Playback::Loop(RepeatCount::Finite(0)))),
        Err(AnimationStartError::InvalidRepeatCount)
    );
    assert_eq!(
        timeline.start_descriptor(descriptor(Playback::Alternate(RepeatCount::Finite(
            (1 << 53) + 1,
        )))),
        Err(AnimationStartError::UnrepresentableRepeatCount)
    );
    assert!(matches!(
        timeline.start(Animation::new(
            Unsupported(0.0),
            Unsupported(1.0),
            1.0,
            Easing::Linear,
        )),
        Err(AnimationStartError::UnsupportedPayload { .. })
    ));

    let admitted = timeline
        .start(Animation::new(0.0f32, 1.0, 1.0, Easing::Linear))
        .unwrap();
    assert_eq!(
        timeline.start(Animation::new(1.0f32, 2.0, 1.0, Easing::Linear)),
        Err(AnimationStartError::CapacityExhausted { capacity: 1 })
    );
    assert_eq!(
        timeline.sample::<f32>(admitted),
        AnimationSample::Active(0.0)
    );
}

#[test]
fn timeline_contract_baseline_payload_set_is_explicitly_admitted() {
    let mut timeline = Timeline::with_capacity(4);
    timeline.update(0.0).unwrap();

    let f32_handle = timeline
        .start(Animation::new(1.0f32, 2.0, 1.0, Easing::Linear))
        .unwrap();
    let f64_handle = timeline
        .start(Animation::new(3.0f64, 4.0, 1.0, Easing::Linear))
        .unwrap();
    let point = Point::new(5.0, 6.0);
    let point_handle = timeline
        .start(Animation::new(
            point,
            Point::new(7.0, 8.0),
            1.0,
            Easing::Linear,
        ))
        .unwrap();
    let color = Color::from_linear(0.1, 0.2, 0.3, 1.0);
    let color_handle = timeline
        .start(Animation::new(
            color,
            Color::from_linear(0.4, 0.5, 0.6, 1.0),
            1.0,
            Easing::Linear,
        ))
        .unwrap();

    assert_eq!(
        timeline.sample::<f32>(f32_handle),
        AnimationSample::Active(1.0)
    );
    assert_eq!(
        timeline.sample::<f64>(f64_handle),
        AnimationSample::Active(3.0)
    );
    assert_eq!(
        timeline.sample::<Point>(point_handle),
        AnimationSample::Active(point)
    );
    assert_eq!(
        timeline.sample::<Color>(color_handle),
        AnimationSample::Active(color)
    );
}

#[test]
fn timeline_contract_same_frame_start_is_exact_and_regressed_time_is_atomic() {
    let mut timeline = Timeline::with_capacity(2);
    timeline.update(20.0).unwrap();
    let handle = timeline
        .start(Animation::new(4.0f32, 8.0, 1.0, Easing::Linear))
        .unwrap();

    assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Active(4.0));
    assert_eq!(
        timeline.update(19.0),
        Err(TimelineAdvanceError::TimestampRegressed {
            previous: 20.0,
            current: 19.0,
        })
    );
    assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Active(4.0));

    timeline.update(20.25).unwrap();
    assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Active(5.0));
}

#[test]
fn timeline_contract_update_context_can_start_and_sample_at_the_captured_frame_time() {
    let mut timeline = Timeline::with_capacity(2);
    timeline.update(7.0).unwrap();

    let mut update = UpdateContext::new(Some(AnimationContext::new(&mut timeline)));
    let handle = update
        .start_animation_descriptor(descriptor(Playback::Loop(RepeatCount::Infinite)))
        .unwrap()
        .unwrap();

    assert_eq!(
        update.sample_animation::<f32>(handle),
        Some(AnimationSample::Active(0.0))
    );
    update.cancel_animation(handle);
    assert_eq!(
        update.sample_animation::<f32>(handle),
        Some(AnimationSample::Cancelled)
    );
}

#[test]
fn timeline_contract_sixty_and_one_hundred_twenty_hertz_match_at_shared_timestamps() {
    let mut sixty = Timeline::with_capacity(1);
    let mut one_twenty = Timeline::with_capacity(1);
    sixty.update(0.0).unwrap();
    one_twenty.update(0.0).unwrap();

    let sixty_handle = sixty
        .start_descriptor(descriptor(Playback::Alternate(RepeatCount::Infinite)))
        .unwrap();
    let one_twenty_handle = one_twenty
        .start_descriptor(descriptor(Playback::Alternate(RepeatCount::Infinite)))
        .unwrap();

    for tick in 1..=120 {
        let timestamp = tick as f64 / 120.0;
        one_twenty.update(timestamp).unwrap();
        if tick % 2 == 0 {
            sixty.update(timestamp).unwrap();
            assert_eq!(
                sixty.sample::<f32>(sixty_handle),
                one_twenty.sample::<f32>(one_twenty_handle)
            );
        }
    }
}

#[test]
fn timeline_contract_repeated_lifecycle_stays_within_the_declared_storage_bound() {
    let mut timeline = Timeline::with_capacity(2);
    timeline.update(0.0).unwrap();

    for frame in 1..=2048 {
        let handle = timeline
            .start(Animation::new(0.0f32, 1.0, 1.0, Easing::Linear))
            .unwrap();
        timeline.cancel(handle);
        assert!(timeline.retained_entry_count() <= timeline.capacity());
        timeline.update(frame as f64).unwrap();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Missing);
        assert!(timeline.retained_entry_count() <= timeline.capacity());
    }
}
