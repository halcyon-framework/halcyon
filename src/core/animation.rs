// SPEC-HAL-021: engine-owned, absolute-time value interpolation.
//
// Timeline advances once from the captured engine timestamp. Starts and cancels
// bind to that timestamp, sampling is read-only for the rest of the frame, and
// terminal observations retire at the next frame boundary. Timeline owns no
// gesture, layout, application, renderer, or scheduling authority.

use std::any::{Any, type_name};
use std::sync::Arc;

#[cfg(debug_assertions)]
use std::cell::Cell;

use crate::core::math::Point;
use crate::core::paint::Color;

/// Default finite storage policy for one engine-owned Timeline.
pub const DEFAULT_TIMELINE_CAPACITY: usize = 4096;

const MAX_EXACT_LEG_COUNT: u64 = 1 << 53;

// ============================================================================
// Debug Phase Tracking (ANIM-INV-2)
// ============================================================================

#[cfg(debug_assertions)]
thread_local! {
    /// True after layout begins, false after timeline.update() completes.
    /// Used to enforce ANIM-INV-2: no timeline mutation after layout begins.
    static LAYOUT_PHASE_ACTIVE: Cell<bool> = const { Cell::new(false) };
}

/// DEBUG: Mark layout phase as started. Called at start of layout_tree().
#[cfg(debug_assertions)]
pub fn debug_mark_layout_started() {
    LAYOUT_PHASE_ACTIVE.with(|f| f.set(true));
}

/// DEBUG: Mark layout phase as ended. Called after paint_tree().
#[cfg(debug_assertions)]
pub fn debug_mark_layout_ended() {
    LAYOUT_PHASE_ACTIVE.with(|f| f.set(false));
}

/// DEBUG: Check if layout phase is active.
#[cfg(debug_assertions)]
fn is_layout_phase_active() -> bool {
    LAYOUT_PHASE_ACTIVE.with(|f| f.get())
}

// ============================================================================
// Core Types
// ============================================================================

/// Unique identifier for an animation instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AnimationId(pub u64);

/// Handle to a running animation.
///
/// Widgets store these in their internal state to track active animations.
/// The handle is ID-based (ANIM-INV-5) and does not store references.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransitionHandle {
    pub id: AnimationId,
}

impl TransitionHandle {
    pub fn new(id: AnimationId) -> Self {
        Self { id }
    }
}

/// Opaque identity of the engine owner responsible for an animation lifetime.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AnimationOwnerId {
    Legacy(u64),
    Mounted(crate::core::widget_tree::WidgetMountToken),
}

impl AnimationOwnerId {
    pub(crate) const fn new(raw: u64) -> Self {
        Self::Legacy(raw)
    }

    pub(crate) fn mounted(mount: crate::core::widget_tree::WidgetMountToken) -> Self {
        Self::Mounted(mount)
    }

    fn is_retired(&self) -> bool {
        matches!(self, Self::Mounted(mount) if !mount.is_live())
    }
}

/// Explicit repetition count for a repeating animation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RepeatCount {
    /// A positive, finite number of cycles or legs.
    Finite(u64),
    /// Playback continues until its owner cancels it.
    Infinite,
}

/// Playback policy for one animation descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Playback {
    /// One forward leg from `from` to `to`.
    #[default]
    Once,
    /// Repeated forward legs. Each exact interior boundary restarts at `from`.
    Loop(RepeatCount),
    /// Alternating forward and reverse legs.
    Alternate(RepeatCount),
}

/// Read-only state observed through an animation handle.
#[derive(Debug, Clone, PartialEq)]
pub enum AnimationSample<T> {
    Active(T),
    Completed(T),
    Cancelled,
    Missing,
    TypeMismatch,
}

impl<T> AnimationSample<T> {
    /// Return the sampled value for active and just-completed handles.
    pub fn into_value(self) -> Option<T> {
        match self {
            Self::Active(value) | Self::Completed(value) => Some(value),
            Self::Cancelled | Self::Missing | Self::TypeMismatch => None,
        }
    }
}

/// Failure to admit a descriptor into bounded Timeline storage.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum AnimationStartError {
    #[error("animation duration must be positive and finite")]
    InvalidDuration,
    #[error("cubic-bezier control points must be finite")]
    InvalidEasing,
    #[error("finite playback count must be positive")]
    InvalidRepeatCount,
    #[error("finite playback count cannot be represented exactly")]
    UnrepresentableRepeatCount,
    #[error("finite playback duration cannot be represented")]
    UnrepresentableTotalDuration,
    #[error("animation payload type is not admitted: {type_name}")]
    UnsupportedPayload { type_name: &'static str },
    #[error("animation timeline capacity exhausted ({capacity} entries)")]
    CapacityExhausted { capacity: usize },
    #[error("animation handle identity space exhausted")]
    IdExhausted,
}

/// Failure to advance Timeline from the engine timestamp.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum TimelineAdvanceError {
    #[error("animation timestamp must be finite")]
    NonFiniteTimestamp,
    #[error("animation timestamp regressed from {previous} to {current}")]
    TimestampRegressed { previous: f64, current: f64 },
    #[error("animation elapsed leg quotient cannot be represented exactly")]
    UnrepresentableElapsed,
}

/// Easing function for animation interpolation.
///
/// Easing transforms the linear progress `t` (0..1) into a curved value.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    /// Linear interpolation (no easing).
    Linear,
    /// Ease in (slow start, fast end).
    EaseIn,
    /// Ease out (fast start, slow end).
    EaseOut,
    /// Ease in-out (slow start and end).
    EaseInOut,
    /// Cubic bezier curve with control points.
    CubicBezier { p1: (f32, f32), p2: (f32, f32) },
}

impl Easing {
    /// Apply easing function to linear progress t (0..1).
    /// Returns eased progress (0..1).
    #[inline]
    pub fn apply(&self, t: f32) -> f32 {
        let t = t.clamp(0.0, 1.0);
        match self {
            Easing::Linear => t,
            Easing::EaseIn => t * t,
            Easing::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            Easing::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(2) / 2.0
                }
            }
            Easing::CubicBezier { p1, p2 } => cubic_bezier_y(t, p1.0, p1.1, p2.0, p2.1),
        }
    }

    fn is_finite(self) -> bool {
        match self {
            Self::Linear | Self::EaseIn | Self::EaseOut | Self::EaseInOut => true,
            Self::CubicBezier { p1, p2 } => {
                p1.0.is_finite() && p1.1.is_finite() && p2.0.is_finite() && p2.1.is_finite()
            }
        }
    }
}

/// Evaluate cubic bezier Y value for a given T parameter.
/// Control points: (0,0), (x1,y1), (x2,y2), (1,1)
/// This is a simplified evaluation - for production, use iterative solve for x→t mapping.
#[inline]
fn cubic_bezier_y(t: f32, _x1: f32, y1: f32, _x2: f32, y2: f32) -> f32 {
    // For now, use direct t→y mapping (assumes x≈t, which is approximate).
    // A proper implementation would solve for t given x, then evaluate y(t).
    // This is acceptable for common UI curves where x1≈0.4, x2≈0.6.
    let t2 = t * t;
    let t3 = t2 * t;
    let mt = 1.0 - t;
    let mt2 = mt * mt;
    let _mt3 = mt2 * mt;

    // Bezier: (1-t)³*0 + 3*(1-t)²*t*y1 + 3*(1-t)*t²*y2 + t³*1
    3.0 * mt2 * t * y1 + 3.0 * mt * t2 * y2 + t3
}

// ============================================================================
// Interpolatable Trait
// ============================================================================

/// Trait for types that can be interpolated between two values.
///
/// SPEC-HAL-021 §5.3: Required implementations for f32, f64, Point (Vec2), Color.
pub trait Interpolatable: Clone + Send + 'static {
    /// Linear interpolation from `a` to `b` at progress `t` (0..1).
    fn lerp(a: &Self, b: &Self, t: f32) -> Self;
}

impl Interpolatable for f32 {
    #[inline]
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * t
    }
}

impl Interpolatable for f64 {
    #[inline]
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        a + (b - a) * (t as f64)
    }
}

impl Interpolatable for Point {
    #[inline]
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
    }
}

impl Interpolatable for Color {
    #[inline]
    fn lerp(a: &Self, b: &Self, t: f32) -> Self {
        // Linear interpolation in linear color space
        Color::from_linear(
            a.r + (b.r - a.r) * t,
            a.g + (b.g - a.g) * t,
            a.b + (b.b - a.b) * t,
            a.a + (b.a - a.a) * t,
        )
    }
}

// ============================================================================
// Animation Definition
// ============================================================================

/// Animation definition: interpolates from `from` to `to` over `duration` using `easing`.
///
/// This is the input to `Timeline::start()`. The Timeline stores an internal
/// representation (ActiveAnimation) that tracks runtime state.
#[derive(Debug, Clone)]
pub struct Animation<T: Interpolatable> {
    /// Starting value
    pub from: T,
    /// Ending value
    pub to: T,
    /// Duration in seconds (must be > 0)
    pub duration: f32,
    /// Easing function
    pub easing: Easing,
}

impl<T: Interpolatable> Animation<T> {
    pub fn new(from: T, to: T, duration: f32, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            easing,
        }
    }

    /// Combine this interpolation leg with an explicit playback policy.
    pub fn with_playback(self, playback: Playback) -> AnimationDescriptor<T> {
        AnimationDescriptor {
            animation: self,
            playback,
        }
    }
}

/// Complete Timeline descriptor: interpolation endpoints plus playback policy.
#[derive(Debug, Clone)]
pub struct AnimationDescriptor<T: Interpolatable> {
    pub animation: Animation<T>,
    pub playback: Playback,
}

impl<T: Interpolatable> AnimationDescriptor<T> {
    pub fn new(animation: Animation<T>, playback: Playback) -> Self {
        Self {
            animation,
            playback,
        }
    }

    pub fn once(animation: Animation<T>) -> Self {
        Self::new(animation, Playback::Once)
    }
}

impl<T: Interpolatable> From<Animation<T>> for AnimationDescriptor<T> {
    fn from(animation: Animation<T>) -> Self {
        Self::once(animation)
    }
}

// ============================================================================
// Bounded Timeline Storage
// ============================================================================

#[derive(Clone)]
enum AnimationValue {
    F32(f32),
    F64(f64),
    Point(Point),
    Color(Color),
}

impl AnimationValue {
    fn admit<T: Interpolatable>(value: &T) -> Option<Self> {
        let value = value as &dyn Any;
        if let Some(value) = value.downcast_ref::<f32>() {
            Some(Self::F32(*value))
        } else if let Some(value) = value.downcast_ref::<f64>() {
            Some(Self::F64(*value))
        } else if let Some(value) = value.downcast_ref::<Point>() {
            Some(Self::Point(*value))
        } else {
            value.downcast_ref::<Color>().copied().map(Self::Color)
        }
    }

    fn interpolate(&self, to: &Self, progress: f32) -> Self {
        match (self, to) {
            (Self::F32(from), Self::F32(to)) => Self::F32(f32::lerp(from, to, progress)),
            (Self::F64(from), Self::F64(to)) => Self::F64(f64::lerp(from, to, progress)),
            (Self::Point(from), Self::Point(to)) => Self::Point(Point::lerp(from, to, progress)),
            (Self::Color(from), Self::Color(to)) => {
                Self::Color(<Color as Interpolatable>::lerp(from, to, progress))
            }
            _ => unreachable!("Timeline admitted mismatched endpoint types"),
        }
    }

    fn as_any(&self) -> &dyn Any {
        match self {
            Self::F32(value) => value,
            Self::F64(value) => value,
            Self::Point(value) => value,
            Self::Color(value) => value,
        }
    }

    fn get<T: Interpolatable>(&self) -> Option<T> {
        self.as_any().downcast_ref::<T>().cloned()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EntryState {
    Active,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy)]
enum PlaybackPosition {
    Active { progress: f32, reverse: bool },
    Completed { terminal_is_to: bool },
}

struct TimelineEntry {
    id: AnimationId,
    owner: Option<AnimationOwnerId>,
    from: AnimationValue,
    to: AnimationValue,
    duration: f64,
    easing: Easing,
    playback: Playback,
    start_time: f64,
    current: AnimationValue,
    state: EntryState,
}

impl TimelineEntry {
    fn new<T: Interpolatable>(
        id: AnimationId,
        owner: Option<AnimationOwnerId>,
        descriptor: AnimationDescriptor<T>,
        start_time: f64,
    ) -> Result<Self, AnimationStartError> {
        let AnimationDescriptor {
            animation,
            playback,
        } = descriptor;
        validate_descriptor(&animation, playback)?;

        let from = AnimationValue::admit(&animation.from).ok_or(
            AnimationStartError::UnsupportedPayload {
                type_name: type_name::<T>(),
            },
        )?;
        let to = AnimationValue::admit(&animation.to).ok_or(
            AnimationStartError::UnsupportedPayload {
                type_name: type_name::<T>(),
            },
        )?;
        let current = from.clone();

        Ok(Self {
            id,
            owner,
            from,
            to,
            duration: f64::from(animation.duration),
            easing: animation.easing,
            playback,
            start_time,
            current,
            state: EntryState::Active,
        })
    }

    fn position(&self, now_secs: f64) -> Result<PlaybackPosition, TimelineAdvanceError> {
        let elapsed = (now_secs - self.start_time).max(0.0);
        match self.playback {
            Playback::Once => {
                if elapsed >= self.duration {
                    Ok(PlaybackPosition::Completed {
                        terminal_is_to: true,
                    })
                } else {
                    Ok(PlaybackPosition::Active {
                        progress: (elapsed / self.duration) as f32,
                        reverse: false,
                    })
                }
            }
            Playback::Loop(count) => self.repeating_position(elapsed, count, false),
            Playback::Alternate(count) => self.repeating_position(elapsed, count, true),
        }
    }

    fn repeating_position(
        &self,
        elapsed: f64,
        count: RepeatCount,
        alternate: bool,
    ) -> Result<PlaybackPosition, TimelineAdvanceError> {
        if let RepeatCount::Finite(count) = count {
            let total_duration = self.duration * count as f64;
            if elapsed >= total_duration {
                return Ok(PlaybackPosition::Completed {
                    terminal_is_to: !alternate || count % 2 == 1,
                });
            }
        }

        let quotient = elapsed / self.duration;
        if !quotient.is_finite() || quotient > MAX_EXACT_LEG_COUNT as f64 {
            return Err(TimelineAdvanceError::UnrepresentableElapsed);
        }

        let leg = quotient.floor() as u64;
        let progress = (quotient - leg as f64) as f32;
        Ok(PlaybackPosition::Active {
            progress,
            reverse: alternate && leg % 2 == 1,
        })
    }

    fn apply_position(&mut self, position: PlaybackPosition) {
        match position {
            PlaybackPosition::Active { progress, reverse } => {
                let eased = self.easing.apply(progress);
                self.current = if reverse {
                    self.to.interpolate(&self.from, eased)
                } else {
                    self.from.interpolate(&self.to, eased)
                };
            }
            PlaybackPosition::Completed { terminal_is_to } => {
                self.current = if terminal_is_to {
                    self.to.clone()
                } else {
                    self.from.clone()
                };
                self.state = EntryState::Completed;
            }
        }
    }
}

fn validate_descriptor<T: Interpolatable>(
    animation: &Animation<T>,
    playback: Playback,
) -> Result<(), AnimationStartError> {
    if !animation.duration.is_finite() || animation.duration <= 0.0 {
        return Err(AnimationStartError::InvalidDuration);
    }
    if !animation.easing.is_finite() {
        return Err(AnimationStartError::InvalidEasing);
    }

    let count = match playback {
        Playback::Once => return Ok(()),
        Playback::Loop(count) | Playback::Alternate(count) => count,
    };
    let RepeatCount::Finite(count) = count else {
        return Ok(());
    };
    if count == 0 {
        return Err(AnimationStartError::InvalidRepeatCount);
    }
    if count > MAX_EXACT_LEG_COUNT {
        return Err(AnimationStartError::UnrepresentableRepeatCount);
    }
    if !(f64::from(animation.duration) * count as f64).is_finite() {
        return Err(AnimationStartError::UnrepresentableTotalDuration);
    }
    Ok(())
}

// ============================================================================
// Timeline
// ============================================================================

/// Equality-only identity of one issuer, retained by trusted engine custody.
pub(crate) struct TimelineIssuerToken(Arc<()>);

impl Clone for TimelineIssuerToken {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl PartialEq for TimelineIssuerToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for TimelineIssuerToken {}

/// Engine-owned, bounded registry of frame-stable animation samples.
pub struct Timeline {
    issuer: TimelineIssuerToken,
    next_id: Option<u64>,
    entries: Vec<TimelineEntry>,
    capacity: usize,
    last_update_time: Option<f64>,
}

impl Default for Timeline {
    fn default() -> Self {
        Self::new()
    }
}

impl Timeline {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_TIMELINE_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            issuer: TimelineIssuerToken(Arc::new(())),
            next_id: Some(1),
            entries: Vec::with_capacity(capacity),
            capacity,
            last_update_time: None,
        }
    }

    pub fn capacity(&self) -> usize {
        self.capacity
    }

    /// Engine custody proof; independent of this value's movable address.
    pub(crate) fn issuer(&self) -> TimelineIssuerToken {
        self.issuer.clone()
    }

    /// Admit a single forward animation.
    pub fn start<T: Interpolatable>(
        &mut self,
        animation: Animation<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.start_descriptor(AnimationDescriptor::once(animation))
    }

    /// Admit a complete descriptor with an explicit playback policy.
    pub fn start_descriptor<T: Interpolatable>(
        &mut self,
        descriptor: AnimationDescriptor<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.start_descriptor_for_owner(None, descriptor)
    }

    fn start_descriptor_for_owner<T: Interpolatable>(
        &mut self,
        owner: Option<AnimationOwnerId>,
        descriptor: AnimationDescriptor<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            !is_layout_phase_active(),
            "ANIM-INV-2: Cannot start animation after layout begins"
        );

        if self.entries.len() >= self.capacity {
            return Err(AnimationStartError::CapacityExhausted {
                capacity: self.capacity,
            });
        }
        let raw_id = self.next_id.ok_or(AnimationStartError::IdExhausted)?;
        let id = AnimationId(raw_id);
        let entry =
            TimelineEntry::new(id, owner, descriptor, self.last_update_time.unwrap_or(0.0))?;

        self.entries.push(entry);
        self.next_id = raw_id.checked_add(1);
        Ok(TransitionHandle::new(id))
    }

    /// Mark a live handle cancelled for the remainder of the current frame.
    pub fn cancel(&mut self, handle: TransitionHandle) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !is_layout_phase_active(),
            "ANIM-INV-2: Cannot cancel animation after layout begins"
        );

        if let Some(entry) = self.entries.iter_mut().find(|entry| entry.id == handle.id)
            && entry.state == EntryState::Active
        {
            entry.state = EntryState::Cancelled;
        }
    }

    /// Cancel every live handle registered to one engine owner.
    pub(crate) fn cancel_owner(&mut self, owner: AnimationOwnerId) {
        #[cfg(debug_assertions)]
        debug_assert!(
            !is_layout_phase_active(),
            "ANIM-INV-2: Cannot cancel animation after layout begins"
        );

        for entry in &mut self.entries {
            if entry.owner.as_ref() == Some(&owner) && entry.state == EntryState::Active {
                entry.state = EntryState::Cancelled;
            }
        }
    }

    /// Reconcile concrete instances at the existing engine Update boundary.
    /// This never cancels a successor sharing the predecessor's TreeWidgetId.
    pub(crate) fn cancel_retired_widget_owners(&mut self) {
        for entry in &mut self.entries {
            if entry.state == EntryState::Active
                && entry
                    .owner
                    .as_ref()
                    .is_some_and(AnimationOwnerId::is_retired)
            {
                entry.state = EntryState::Cancelled;
            }
        }
    }

    /// Advance existing handles once from the captured engine timestamp.
    ///
    /// This operation performs no heap allocation. Terminal records from the
    /// previous frame retire only after the incoming timestamp is validated.
    pub fn update(&mut self, now_secs: f64) -> Result<(), TimelineAdvanceError> {
        #[cfg(debug_assertions)]
        debug_assert!(
            !is_layout_phase_active(),
            "ANIM-INV-2: Cannot advance Timeline after layout begins"
        );

        if !now_secs.is_finite() {
            return Err(TimelineAdvanceError::NonFiniteTimestamp);
        }
        if let Some(previous) = self.last_update_time
            && now_secs < previous
        {
            return Err(TimelineAdvanceError::TimestampRegressed {
                previous,
                current: now_secs,
            });
        }

        for entry in &self.entries {
            if entry.state == EntryState::Active
                && !entry
                    .owner
                    .as_ref()
                    .is_some_and(AnimationOwnerId::is_retired)
            {
                entry.position(now_secs)?;
            }
        }

        self.entries
            .retain(|entry| entry.state == EntryState::Active);
        self.cancel_retired_widget_owners();
        for entry in &mut self.entries {
            if entry.state == EntryState::Active {
                let position = entry.position(now_secs)?;
                entry.apply_position(position);
            }
        }
        self.last_update_time = Some(now_secs);
        Ok(())
    }

    /// Sample a handle without collapsing lifecycle or type state.
    pub fn sample<T: Interpolatable>(&self, handle: TransitionHandle) -> AnimationSample<T> {
        let Some(entry) = self.entries.iter().find(|entry| entry.id == handle.id) else {
            return AnimationSample::Missing;
        };

        match entry.state {
            EntryState::Cancelled => AnimationSample::Cancelled,
            EntryState::Active
                if entry
                    .owner
                    .as_ref()
                    .is_some_and(AnimationOwnerId::is_retired) =>
            {
                AnimationSample::Cancelled
            }
            EntryState::Active => entry
                .current
                .get::<T>()
                .map(AnimationSample::Active)
                .unwrap_or(AnimationSample::TypeMismatch),
            EntryState::Completed => entry
                .current
                .get::<T>()
                .map(AnimationSample::Completed)
                .unwrap_or(AnimationSample::TypeMismatch),
        }
    }

    /// Value-only read for finite paint consumers.
    ///
    /// Both active and completion-frame terminal values are returned. New code
    /// that needs lifecycle policy should use [`Timeline::sample`].
    pub fn get<T: Interpolatable>(&self, handle: TransitionHandle) -> Option<T> {
        self.sample(handle).into_value()
    }

    pub fn is_active(&self, handle: TransitionHandle) -> bool {
        self.entries.iter().any(|entry| {
            entry.id == handle.id
                && entry.state == EntryState::Active
                && !entry
                    .owner
                    .as_ref()
                    .is_some_and(AnimationOwnerId::is_retired)
        })
    }

    pub fn has_active_animations(&self) -> bool {
        self.entries.iter().any(|entry| {
            entry.state == EntryState::Active
                && !entry
                    .owner
                    .as_ref()
                    .is_some_and(AnimationOwnerId::is_retired)
        })
    }

    pub fn active_count(&self) -> usize {
        self.entries
            .iter()
            .filter(|entry| {
                entry.state == EntryState::Active
                    && !entry
                        .owner
                        .as_ref()
                        .is_some_and(AnimationOwnerId::is_retired)
            })
            .count()
    }

    pub fn retained_entry_count(&self) -> usize {
        self.entries.len()
    }
}

// ============================================================================
// AnimationContext (Stage 2.5: Capability Wrapper)
// ============================================================================

/// Narrow capability wrapper for animation start operations.
///
/// This struct encapsulates mutable access to the engine-owned Timeline for
/// starting, cancelling, and sampling animations without exposing registry
/// mutation to consumers.
///
/// SPEC-HAL-021: Animation is a utility, not a first-class routing primitive.
/// This wrapper keeps animation-start capability isolated and optional.
pub struct AnimationContext<'a> {
    timeline: &'a mut Timeline,
}

impl<'a> AnimationContext<'a> {
    /// Frozen timestamp of this engine Timeline frame, never another clock.
    pub(crate) fn frame_timestamp_secs(&self) -> Option<f64> {
        self.timeline.last_update_time
    }
    /// Create a new AnimationContext wrapping a mutable Timeline reference.
    pub fn new(timeline: &'a mut Timeline) -> Self {
        Self { timeline }
    }

    pub(crate) fn reborrow(&mut self) -> AnimationContext<'_> {
        AnimationContext::new(&mut *self.timeline)
    }

    pub(crate) fn cancel_retired_widget_owners(&mut self) {
        self.timeline.cancel_retired_widget_owners();
    }

    pub(crate) fn is_active(&self, handle: TransitionHandle) -> bool {
        self.timeline.is_active(handle)
    }

    /// Start a `Once` animation with explicit admission failure.
    pub fn start<T: Interpolatable>(
        &mut self,
        animation: Animation<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.timeline.start(animation)
    }

    pub fn start_descriptor<T: Interpolatable>(
        &mut self,
        descriptor: AnimationDescriptor<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.timeline.start_descriptor(descriptor)
    }

    pub(crate) fn start_descriptor_for_owner<T: Interpolatable>(
        &mut self,
        owner: AnimationOwnerId,
        descriptor: AnimationDescriptor<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.timeline
            .start_descriptor_for_owner(Some(owner), descriptor)
    }

    pub(crate) fn start_for_owner<T: Interpolatable>(
        &mut self,
        owner: AnimationOwnerId,
        animation: Animation<T>,
    ) -> Result<TransitionHandle, AnimationStartError> {
        self.start_descriptor_for_owner(owner, AnimationDescriptor::once(animation))
    }

    /// Cancel an animation by handle.
    pub fn cancel(&mut self, handle: TransitionHandle) {
        self.timeline.cancel(handle);
    }

    pub fn sample<T: Interpolatable>(&self, handle: TransitionHandle) -> AnimationSample<T> {
        self.timeline.sample(handle)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::EngineTicker;
    use crate::core::widget::UpdateContext;
    use crate::utils::id::TreeWidgetId;
    use std::time::{Duration, Instant};

    #[test]
    fn timeline_contract_removed_owner_cancels_its_infinite_animation() {
        let mut timeline = Timeline::with_capacity(2);
        timeline.update(0.0).unwrap();
        let owner = TreeWidgetId(41);
        let handle = {
            let mut update = UpdateContext::new(Some(AnimationContext::new(&mut timeline)));
            update.begin_widget_update_for_owner(owner);
            update
                .start_animation_descriptor(AnimationDescriptor::new(
                    Animation::new(0.0f32, 1.0, 1.0, Easing::Linear),
                    Playback::Loop(RepeatCount::Infinite),
                ))
                .unwrap()
                .unwrap()
        };

        timeline.cancel_owner(AnimationOwnerId::new(owner.0));
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Cancelled);
        assert!(!timeline.has_active_animations());
        timeline.update(0.25).unwrap();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Missing);
    }

    #[test]
    fn timeline_contract_concrete_event_owner_replacement_isolated_from_successor() {
        use crate::core::event::KeyboardAuthorityMode;
        use crate::core::widget::EventContext;
        use crate::core::widget_tree::WidgetTree;
        use crate::widgets::label::Label;

        let mut timeline = Timeline::with_capacity(4);
        timeline.update(0.0).unwrap();
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(Label::new("old")), None, 0);
        let owner = tree.node(node).unwrap().id();
        let start = |tree: &mut WidgetTree, timeline: &mut Timeline| {
            let mut cx = EventContext::new(
                owner,
                tree,
                0,
                std::ptr::null_mut(),
                None,
                Some(AnimationContext::new(timeline)),
                false,
                false,
                KeyboardAuthorityMode::Navigation,
            );
            cx.start_animation_descriptor(AnimationDescriptor::new(
                Animation::new(0.0f32, 1.0, 1.0, Easing::Linear),
                Playback::Loop(RepeatCount::Infinite),
            ))
            .unwrap()
            .unwrap()
        };
        let old = start(&mut tree, &mut timeline);
        tree.replace_widget_only(node, Box::new(Label::new("new")));
        assert_eq!(tree.node(node).unwrap().id(), owner);
        let successor = start(&mut tree, &mut timeline);
        assert_ne!(old, successor);
        timeline.cancel_owner(AnimationOwnerId::new(owner.0));
        timeline.cancel_retired_widget_owners();
        assert_eq!(timeline.sample::<f32>(old), AnimationSample::Cancelled);
        assert_eq!(
            timeline.sample::<f32>(successor),
            AnimationSample::Active(0.0)
        );
        assert_eq!(timeline.active_count(), 1);
        timeline.update(0.25).unwrap();
        assert_eq!(timeline.sample::<f32>(old), AnimationSample::Missing);
        assert_eq!(
            timeline.sample::<f32>(successor),
            AnimationSample::Active(0.25)
        );
        tree.remove_node(node);
        timeline.cancel_retired_widget_owners();
        assert_eq!(
            timeline.sample::<f32>(successor),
            AnimationSample::Cancelled
        );
        assert!(!timeline.has_active_animations());
        timeline.update(0.5).unwrap();
        assert_eq!(timeline.sample::<f32>(successor), AnimationSample::Missing);
    }
    #[test]
    fn timeline_contract_engine_clock_preserves_sub_millisecond_and_120_hz_time() {
        let epoch = Instant::now();
        let ticker = EngineTicker::new(epoch);
        let first_instant = epoch + Duration::from_nanos(8_100_000);
        let second_instant = epoch + Duration::from_nanos(8_900_000);

        assert_eq!(
            ticker.frame_time_ms(first_instant),
            ticker.frame_time_ms(second_instant),
            "the compatibility millisecond clock intentionally cannot distinguish these instants"
        );

        let mut timeline = Timeline::with_capacity(1);
        timeline.update(ticker.timeline_time_secs(epoch)).unwrap();
        let handle = timeline
            .start(Animation::new(0.0f64, 1.0, 1.0, Easing::Linear))
            .unwrap();
        timeline
            .update(ticker.timeline_time_secs(first_instant))
            .unwrap();
        let first_sample = match timeline.sample::<f64>(handle) {
            AnimationSample::Active(value) => value,
            sample => panic!("expected active first sample, got {sample:?}"),
        };
        timeline
            .update(ticker.timeline_time_secs(second_instant))
            .unwrap();
        let second_sample = match timeline.sample::<f64>(handle) {
            AnimationSample::Active(value) => value,
            sample => panic!("expected active second sample, got {sample:?}"),
        };
        assert!(second_sample > first_sample);

        let mut previous = ticker.timeline_time_secs(epoch);
        for tick in 1..=120 {
            let instant = epoch + Duration::from_secs_f64(tick as f64 / 120.0);
            let current = ticker.timeline_time_secs(instant);
            let step = current - previous;
            assert!((step - 1.0 / 120.0).abs() <= 1.1e-9);
            assert!((step - 0.008).abs() > 0.000_3);
            assert!((step - 0.009).abs() > 0.000_3);
            previous = current;
        }
    }

    #[test]
    fn test_easing_linear() {
        assert!((Easing::Linear.apply(0.0) - 0.0).abs() < 0.001);
        assert!((Easing::Linear.apply(0.5) - 0.5).abs() < 0.001);
        assert!((Easing::Linear.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_in() {
        assert!((Easing::EaseIn.apply(0.0) - 0.0).abs() < 0.001);
        assert!(Easing::EaseIn.apply(0.5) < 0.5); // Slower than linear
        assert!((Easing::EaseIn.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_easing_ease_out() {
        assert!((Easing::EaseOut.apply(0.0) - 0.0).abs() < 0.001);
        assert!(Easing::EaseOut.apply(0.5) > 0.5); // Faster than linear
        assert!((Easing::EaseOut.apply(1.0) - 1.0).abs() < 0.001);
    }

    #[test]
    fn cubic_bezier_preserves_finite_overshoot() {
        let easing = Easing::CubicBezier {
            p1: (0.25, 2.0),
            p2: (0.75, 2.0),
        };
        assert!(easing.apply(0.5) > 1.0);
    }

    #[test]
    fn test_interpolatable_f32() {
        assert!((f32::lerp(&0.0, &10.0, 0.0) - 0.0).abs() < 0.001);
        assert!((f32::lerp(&0.0, &10.0, 0.5) - 5.0).abs() < 0.001);
        assert!((f32::lerp(&0.0, &10.0, 1.0) - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_interpolatable_point() {
        let a = Point::new(0.0, 0.0);
        let b = Point::new(10.0, 20.0);
        let mid = Point::lerp(&a, &b, 0.5);
        assert!((mid.x - 5.0).abs() < 0.001);
        assert!((mid.y - 10.0).abs() < 0.001);
    }

    #[test]
    fn test_timeline_basic() {
        let mut timeline = Timeline::new();

        let anim = Animation::new(0.0f32, 100.0f32, 1.0, Easing::Linear);
        let handle = timeline.start(anim).unwrap();

        assert!(timeline.is_active(handle));

        // At t=0, value should be 0
        timeline.update(0.0).unwrap();
        let value: Option<f32> = timeline.get(handle);
        assert!(value.is_some());
        assert!((value.unwrap() - 0.0).abs() < 0.001);

        // At t=0.5, value should be 50
        timeline.update(0.5).unwrap();
        let value: Option<f32> = timeline.get(handle);
        assert!((value.unwrap() - 50.0).abs() < 0.001);

        // At t=1.0, animation should complete
        timeline.update(1.0).unwrap();
        // Completion no longer requests another animation frame.
        assert!(!timeline.is_active(handle));
    }

    #[test]
    fn test_timeline_cancel() {
        let mut timeline = Timeline::new();

        let anim = Animation::new(0.0f32, 100.0f32, 1.0, Easing::Linear);
        let handle = timeline.start(anim).unwrap();

        assert!(timeline.is_active(handle));
        timeline.cancel(handle);
        assert!(!timeline.is_active(handle));
    }

    #[test]
    fn test_timeline_multiple_animations() {
        let mut timeline = Timeline::new();

        let anim1 = Animation::new(0.0f32, 100.0f32, 1.0, Easing::Linear);
        let anim2 = Animation::new(0.0f32, 200.0f32, 2.0, Easing::Linear);

        let h1 = timeline.start(anim1).unwrap();
        let h2 = timeline.start(anim2).unwrap();

        timeline.update(0.5).unwrap();

        // anim1: 50% of 100 = 50
        let v1: f32 = timeline.get(h1).unwrap();
        assert!((v1 - 50.0).abs() < 0.001);

        // anim2: 25% of 200 = 50 (0.5s / 2.0s = 0.25)
        let v2: f32 = timeline.get(h2).unwrap();
        assert!((v2 - 50.0).abs() < 0.001);

        // At t=1.0, anim1 should complete but anim2 continues
        timeline.update(1.0).unwrap();
        assert!(!timeline.is_active(h1));
        assert!(timeline.is_active(h2));

        // anim2: 50% of 200 = 100
        let v2: f32 = timeline.get(h2).unwrap();
        assert!((v2 - 100.0).abs() < 0.001);
    }
}
