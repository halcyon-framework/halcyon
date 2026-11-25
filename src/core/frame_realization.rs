//! Immutable observation of canonical application render attempts (SPEC-HAL-361).

use crate::compositor::SurfaceId;
use crate::renderer::{FrameEpoch, TextRealizationFailure, TextRealizationOutcome};
use crate::renderer::{MultiTargetDrawPlan, SurfaceTextRealization};
use std::collections::HashMap;
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// An opaque application-run identity and non-wrapping attempt sequence.
///
/// Equality is meaningful across handles; sequence ordering is meaningful only
/// within one application run. This is neither a renderer epoch nor completion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AppFrameAttemptId {
    run: u64,
    sequence: u64,
}

impl AppFrameAttemptId {
    /// One-based sequence within this application run. Gaps can indicate missed
    /// observations; the observer retains no intermediate history.
    pub const fn sequence(self) -> u64 {
        self.sequence
    }
}

/// Whether this application attempt reached ordinary renderer compilation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AppFrameCompileOutcome {
    Compiled,
    SceneUnchanged,
    LayoutDeferred,
    NoTargets,
    Pending(crate::renderer::RendererPreparationIdentity),
    Failed(crate::renderer::RendererCompileError),
}

/// Mechanical preparation and output facts for one exact surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppSurfaceRealization {
    surface_id: SurfaceId,
    text_realization: Option<SurfaceTextRealization>,
    output_produced: bool,
    submitted: bool,
    presented_called: bool,
    #[cfg(feature = "test-harness")]
    visible_spatial_hud_pairs: Option<u32>,
    #[cfg(feature = "test-harness")]
    ordinary_draw_counts: (usize, usize, usize),
}

impl AppSurfaceRealization {
    pub const fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    /// Historical Compile evidence, including its exact renderer FrameEpoch.
    pub const fn text_realization(&self) -> Option<&SurfaceTextRealization> {
        self.text_realization.as_ref()
    }

    /// Whether `render_multi` returned an actual output for this surface.
    pub const fn output_produced(&self) -> bool {
        self.output_produced
    }

    /// Whether Queue submission was issued for that output; not GPU completion.
    pub const fn submitted(&self) -> bool {
        self.submitted
    }

    /// Whether `Queue::present` was called; not display completion.
    pub const fn presented_called(&self) -> bool {
        self.presented_called
    }

    #[cfg(feature = "test-harness")]
    pub fn visible_spatial_hud_pairs_for_test_harness(&self) -> Option<u32> {
        self.visible_spatial_hud_pairs
    }

    /// Test-only counts of registered Image batches, overlay commands and base graph nodes.
    #[cfg(feature = "test-harness")]
    pub fn ordinary_draw_counts_for_test_harness(&self) -> (usize, usize, usize) {
        self.ordinary_draw_counts
    }
}

/// The immutable result of one application render attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppFrameRealization {
    attempt_id: AppFrameAttemptId,
    compile_outcome: AppFrameCompileOutcome,
    surfaces: Vec<AppSurfaceRealization>,
}

impl AppFrameRealization {
    pub const fn attempt_id(&self) -> AppFrameAttemptId {
        self.attempt_id
    }

    pub const fn compile_outcome(&self) -> &AppFrameCompileOutcome {
        &self.compile_outcome
    }

    /// One record per surface, in the attempt's compositor order.
    pub fn surfaces(&self) -> &[AppSurfaceRealization] {
        &self.surfaces
    }

    pub fn surface(&self, surface_id: SurfaceId) -> Option<&AppSurfaceRealization> {
        self.surfaces
            .iter()
            .find(|surface| surface.surface_id == surface_id)
    }
}

/// Cloneable read-only access to the latest application-attempt observation.
///
/// Obtain this through `AppConfig::with_frame_realization_observer` or the
/// configured `AppContext`. Startup returns None. Publication replaces the prior
/// snapshot, including an explicit pending preparation; there is no queue or history guarantee. Explicitly retained Arc
/// snapshots remain immutable and belong to the retaining consumer.
#[derive(Debug, Clone)]
pub struct AppFrameObserver {
    latest: Arc<Mutex<Option<Arc<AppFrameRealization>>>>,
}

impl AppFrameObserver {
    pub fn latest(&self) -> Option<Arc<AppFrameRealization>> {
        self.latest
            .lock()
            .expect("frame observer lock poisoned")
            .clone()
    }

    pub(crate) fn new() -> Self {
        Self {
            latest: Arc::new(Mutex::new(None)),
        }
    }

    pub(crate) fn publish(&self, receipt: AppFrameRealization) {
        assert!(
            !crate::core::app::in_frame_critical_section(),
            "SPEC-HAL-361: receipt publication must follow the frame critical section"
        );
        let snapshot = Arc::new(receipt);
        *self.latest.lock().expect("frame observer lock poisoned") = Some(snapshot);
    }
}

#[derive(Clone, Copy)]
enum TextFaultNotice {
    First,
    Changed,
    Recovered(AppFrameAttemptId, FrameEpoch),
}

struct RequiredTextFault {
    first_attempt: AppFrameAttemptId,
    first_epoch: FrameEpoch,
    failed_attempt: AppFrameAttemptId,
    failed_epoch: FrameEpoch,
    failure: TextRealizationFailure,
    missing_cluster_count: usize,
    degraded_prepared: bool,
    complete: Option<(AppFrameAttemptId, FrameEpoch)>,
    notice: Option<TextFaultNotice>,
}

/// SPEC-HAL-361 §6: bounded ordinary-path refusal state, independent of observation.
/// Entries exist only for unresolved failures; no text or rendering resources survive here.
#[derive(Default)]
pub(crate) struct RequiredTextFaultGuard {
    faults: HashMap<SurfaceId, RequiredTextFault>,
}

impl RequiredTextFaultGuard {
    pub(crate) fn compiled(&mut self, attempt: AppFrameAttemptId, plan: &MultiTargetDrawPlan) {
        for target in plan.targets() {
            if let Some(text) = plan.text_realization(target.surface_id) {
                self.prepared(attempt, *text);
            }
        }
    }

    fn prepared(&mut self, attempt: AppFrameAttemptId, text: SurfaceTextRealization) {
        match *text.outcome() {
            outcome @ (TextRealizationOutcome::Unavailable(_)
            | TextRealizationOutcome::Degraded { .. }) => {
                let (failure, missing_cluster_count) = match outcome {
                    TextRealizationOutcome::Unavailable(failure) => (failure, 0),
                    TextRealizationOutcome::Degraded {
                        first_failure,
                        missing_cluster_count,
                        ..
                    } => (first_failure, missing_cluster_count),
                    _ => unreachable!(),
                };
                let degraded_prepared = text.is_prepared_for_output();
                // INV-361-TEXT-FAULT-EXPLICIT: first allocation follows an actual refusal.
                let fault = self
                    .faults
                    .entry(text.surface_id())
                    .or_insert(RequiredTextFault {
                        first_attempt: attempt,
                        first_epoch: text.frame_epoch(),
                        failed_attempt: attempt,
                        failed_epoch: text.frame_epoch(),
                        failure,
                        missing_cluster_count,
                        degraded_prepared,
                        complete: None,
                        notice: Some(TextFaultNotice::First),
                    });
                if fault.failure != failure
                    || fault.missing_cluster_count != missing_cluster_count
                    || fault.degraded_prepared != degraded_prepared
                {
                    fault.failure = failure;
                    fault.missing_cluster_count = missing_cluster_count;
                    fault.degraded_prepared = degraded_prepared;
                    fault.notice = Some(TextFaultNotice::Changed);
                }
                fault.failed_attempt = attempt;
                fault.failed_epoch = text.frame_epoch();
                fault.complete = None;
            }
            TextRealizationOutcome::Complete { .. } => {
                if let Some(fault) = self.faults.get_mut(&text.surface_id()) {
                    // INV-361-TEXT-FAULT-RETAINED: Compile alone is not recovery.
                    fault.complete = Some((attempt, text.frame_epoch()));
                }
            }
        }
    }

    pub(crate) fn assert_output_allowed(&self, attempt: AppFrameAttemptId, surface: SurfaceId) {
        assert!(
            !self.refused(attempt, surface),
            "INV-361-TEXT-FAULT-NO-FALSE-OUTPUT: same-Compile refused surface cannot produce or submit output"
        );
    }

    pub(crate) fn refused(&self, attempt: AppFrameAttemptId, surface: SurfaceId) -> bool {
        self.faults
            .get(&surface)
            .is_some_and(|fault| fault.failed_attempt == attempt && !fault.degraded_prepared)
    }

    pub(crate) fn submitted_and_presented(
        &mut self,
        attempt: AppFrameAttemptId,
        surface: SurfaceId,
    ) {
        self.assert_output_allowed(attempt, surface);
        if let Some(fault) = self.faults.get_mut(&surface) {
            if let Some((complete_attempt, epoch)) = fault.complete.filter(|(id, _)| *id == attempt)
            {
                fault.notice = Some(TextFaultNotice::Recovered(complete_attempt, epoch));
            }
        }
    }

    pub(crate) fn retire(&mut self, surface: SurfaceId) {
        self.faults.remove(&surface);
    }

    pub(crate) fn retain_live_surfaces(&mut self, mut is_live: impl FnMut(SurfaceId) -> bool) {
        self.faults.retain(|surface, _| is_live(*surface));
    }

    pub(crate) fn emit_diagnostics(&mut self) {
        if self.faults.values().any(|fault| fault.notice.is_some()) {
            if let Err(error) = self.write_diagnostics(&mut io::stderr().lock()) {
                tracing::error!(
                    ?error,
                    "INV-361-TEXT-FAULT-EXPLICIT: stderr diagnostic write failed"
                );
            }
        }
    }

    pub(crate) fn write_diagnostics(&mut self, writer: &mut impl Write) -> io::Result<()> {
        assert!(
            !crate::core::app::in_frame_critical_section(),
            "INV-361-TEXT-FAULT-EXPLICIT: diagnostic must follow the frame critical section"
        );
        let mut result = Ok(());
        self.faults.retain(|surface, fault| {
            let Some(notice) = fault.notice.take() else { return true; };
            let (event, attempt, epoch) = match notice {
                TextFaultNotice::First => ("first", fault.failed_attempt, fault.failed_epoch),
                TextFaultNotice::Changed => ("changed", fault.failed_attempt, fault.failed_epoch),
                TextFaultNotice::Recovered(attempt, epoch) => ("recovered", attempt, epoch),
            };
            // No formatted history or per-frame buffer; even a closed stderr cannot cause retries.
            let written = writeln!(writer,
                "INV-361-TEXT-FAULT-EXPLICIT event={event} attempt={attempt:?} surface={surface:?} epoch={epoch:?} first_attempt={:?} first_epoch={:?} primitive={} glyph={:?} reason={:?} disposition={} missing_clusters={}",
                fault.first_attempt, fault.first_epoch, fault.failure.primitive_index,
                fault.failure.glyph_index, fault.failure.reason,
                if fault.degraded_prepared {"Degraded"} else {"Unavailable"}, fault.missing_cluster_count);
            if result.is_ok() { result = written; }
            !matches!(notice, TextFaultNotice::Recovered(..))
        });
        result
    }
}

pub(crate) struct AppFrameAttempts {
    run: u64,
    sequence: u64,
}

impl AppFrameAttempts {
    pub(crate) fn new() -> Self {
        static NEXT_RUN: AtomicU64 = AtomicU64::new(1);
        let run = NEXT_RUN
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                next.checked_add(1)
            })
            .expect("SPEC-HAL-361: application run identity exhausted");
        Self { run, sequence: 0 }
    }

    pub(crate) fn next(&mut self) -> AppFrameAttemptId {
        self.sequence = self
            .sequence
            .checked_add(1)
            .expect("SPEC-HAL-361: application frame attempt identity exhausted");
        AppFrameAttemptId {
            run: self.run,
            sequence: self.sequence,
        }
    }
}

/// Exists only while an opted-in canonical attempt is being observed.
pub(crate) struct AppFrameRecorder {
    receipt: AppFrameRealization,
    #[cfg(feature = "test-harness")]
    spatial_hud: Vec<(
        SurfaceId,
        Arc<crate::renderer::wgpu_renderer::spatial_hud::SpatialHudWork>,
    )>,
}

impl AppFrameRecorder {
    pub(crate) fn snapshot(&self) -> AppFrameRealization {
        self.receipt.clone()
    }
    pub(crate) fn new(
        attempt_id: AppFrameAttemptId,
        surfaces: impl Iterator<Item = SurfaceId>,
    ) -> Self {
        Self {
            #[cfg(feature = "test-harness")]
            spatial_hud: Vec::new(),
            receipt: AppFrameRealization {
                attempt_id,
                compile_outcome: AppFrameCompileOutcome::SceneUnchanged,
                surfaces: surfaces
                    .map(|surface_id| AppSurfaceRealization {
                        surface_id,
                        text_realization: None,
                        output_produced: false,
                        submitted: false,
                        presented_called: false,
                        #[cfg(feature = "test-harness")]
                        visible_spatial_hud_pairs: None,
                        #[cfg(feature = "test-harness")]
                        ordinary_draw_counts: (0, 0, 0),
                    })
                    .collect(),
            },
        }
    }

    pub(crate) fn compiled(&mut self, compiled: &MultiTargetDrawPlan) {
        self.receipt.compile_outcome = AppFrameCompileOutcome::Compiled;
        for surface in &mut self.receipt.surfaces {
            surface.text_realization = compiled.text_realization(surface.surface_id).copied();
            #[cfg(feature = "test-harness")]
            if let Some(data) = compiled.surface_compiled_data(surface.surface_id) {
                surface.ordinary_draw_counts = (
                    data.frame_batches
                        .batches
                        .iter()
                        .filter(|batch| {
                            batch.key.pipeline == crate::renderer::batcher::PipelineId::Image
                        })
                        .count(),
                    data.draw_commands
                        .iter()
                        .filter(|command| command.z_index() >= crate::core::paint::LAYER_OVERLAY)
                        .count(),
                    compiled.render_graph_topology.execution_order().len(),
                );
            }
        }
    }

    pub(crate) fn compiled_frame(
        &mut self,
        frame: &crate::renderer::sdf_frame_custody::SdfCompiledFrame,
    ) {
        self.compiled(frame.plan());
        #[cfg(feature = "test-harness")]
        self.spatial_hud.extend(
            frame
                .companion_work()
                .map(|(surface, work)| (surface, work.clone())),
        );
    }

    #[cfg(feature = "test-harness")]
    pub(crate) fn observe_submitted_hud_for_test_harness(
        &mut self,
        renderer: &crate::renderer::wgpu_renderer::WgpuRenderer,
    ) {
        if self.spatial_hud.is_empty() {
            return;
        }
        let gpu = renderer.gpu_resources();
        for (id, work) in self.spatial_hud.drain(..) {
            let surface = self
                .receipt
                .surfaces
                .iter_mut()
                .find(|surface| surface.surface_id == id)
                .unwrap();
            if surface.submitted && surface.presented_called {
                let visible = work.read_visibility_for_test(&gpu).into_iter().sum::<u32>();
                *surface.visible_spatial_hud_pairs.get_or_insert(0) += visible;
            }
        }
    }

    pub(crate) fn unavailable(&mut self, outcome: AppFrameCompileOutcome) {
        self.receipt.compile_outcome = outcome;
    }

    pub(crate) fn output_produced(&mut self, surface_id: SurfaceId) {
        let surface = self
            .receipt
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .expect("SPEC-HAL-361: output surface absent from attempt");
        assert!(
            surface
                .text_realization
                .is_some_and(|text| text.is_prepared_for_output()),
            "SPEC-HAL-361: output requires qualified surface text preparation"
        );
        surface.output_produced = true;
    }

    pub(crate) fn submitted_and_presented(&mut self, surface_id: SurfaceId) {
        let surface = self
            .receipt
            .surfaces
            .iter_mut()
            .find(|surface| surface.surface_id == surface_id)
            .expect("SPEC-HAL-361: submitted surface absent from attempt");
        assert!(
            surface.output_produced,
            "SPEC-HAL-361: submission requires an actual output"
        );
        surface.submitted = true;
        surface.presented_called = true;
    }

    pub(crate) fn finish(self) -> AppFrameRealization {
        self.receipt
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::{AppConfig, AppContext};
    use crate::core::theme::Theme;
    #[test]
    fn frosted_compile_pending_receipt_is_distinct_and_has_no_output() {
        let identity = crate::renderer::RendererPreparationIdentity::new(Arc::new(()));
        identity.add_scope();
        let surface = SurfaceId::test_id(385);
        let mut attempts = AppFrameAttempts::new();
        let mut recorder = AppFrameRecorder::new(attempts.next(), [surface].into_iter());
        recorder.unavailable(AppFrameCompileOutcome::Pending(identity.clone()));
        let pending = recorder.snapshot();
        assert_eq!(
            pending.compile_outcome(),
            &AppFrameCompileOutcome::Pending(identity.clone())
        );
        let record = pending.surface(surface).unwrap();
        assert!(record.text_realization().is_none());
        assert!(!record.output_produced() && !record.submitted() && !record.presented_called());
        identity.finish_scope(false);
        assert_eq!(
            pending.compile_outcome(),
            &AppFrameCompileOutcome::Pending(identity)
        );
        assert_ne!(
            pending.compile_outcome(),
            &AppFrameCompileOutcome::NoTargets
        );
        assert_ne!(
            pending.compile_outcome(),
            &AppFrameCompileOutcome::Failed(crate::renderer::RendererSurfaceError::OutOfMemory.into())
        );
    }

    fn failed_text(surface: SurfaceId, epoch: u64, primitive: usize) -> SurfaceTextRealization {
        SurfaceTextRealization::new(
            surface,
            FrameEpoch(epoch),
            TextRealizationOutcome::Unavailable(TextRealizationFailure {
                primitive_index: primitive,
                glyph_index: Some(0),
                reason: crate::renderer::TextRealizationFailureReason::MissingGlyph,
            }),
        )
    }

    fn complete_text(surface: SurfaceId, epoch: u64) -> SurfaceTextRealization {
        SurfaceTextRealization::new(
            surface,
            FrameEpoch(epoch),
            TextRealizationOutcome::Complete {
                primitive_count: 1,
                glyph_count: 1,
                no_ink_glyph_count: 0,
            },
        )
    }

    #[test]
    fn frame_realization_required_fault_without_observer_retains_and_deduplicates() {
        let context = AppContext::new(Theme::default());
        assert!(context.frame_realization_observer().is_none());
        let mut guard = RequiredTextFaultGuard::default();
        let mut attempts = AppFrameAttempts::new();
        let surface = SurfaceId::test_id(361);
        let first = attempts.next();
        guard.prepared(first, failed_text(surface, 7, 14));
        let mut output = Vec::new();
        guard.write_diagnostics(&mut output).unwrap();
        assert_eq!(String::from_utf8(output).unwrap().lines().count(), 1);
        for epoch in 8..1008 {
            // An unchanged/deferred attempt has no compiled evidence and leaves the guard alone.
            let _skipped = attempts.next();
            guard.prepared(attempts.next(), failed_text(surface, epoch, 14));
            let mut output = Vec::new();
            guard.write_diagnostics(&mut output).unwrap();
            assert!(output.is_empty());
        }
        let fault = &guard.faults[&surface];
        assert_eq!(fault.first_attempt, first);
        assert_eq!(fault.first_epoch, FrameEpoch(7));
        assert_eq!(guard.faults.len(), 1);
    }

    #[test]
    fn frame_realization_required_fault_change_and_submit_only_recovery() {
        let mut guard = RequiredTextFaultGuard::default();
        let mut attempts = AppFrameAttempts::new();
        let surface = SurfaceId::test_id(361);
        let first = attempts.next();
        guard.prepared(first, failed_text(surface, 1, 14));
        guard.write_diagnostics(&mut Vec::new()).unwrap();
        let changed = attempts.next();
        guard.prepared(changed, failed_text(surface, 2, 17));
        let mut output = Vec::new();
        guard.write_diagnostics(&mut output).unwrap();
        assert!(String::from_utf8(output).unwrap().contains("event=changed"));
        assert_eq!(guard.faults[&surface].first_attempt, first);
        let complete = attempts.next();
        guard.prepared(complete, complete_text(surface, 3));
        guard.assert_output_allowed(complete, surface);
        let mut output = Vec::new();
        guard.write_diagnostics(&mut output).unwrap();
        assert!(output.is_empty());
        assert_eq!(
            guard.faults.len(),
            1,
            "Complete without submit is not recovery"
        );
        let _skipped = attempts.next();
        guard.write_diagnostics(&mut output).unwrap();
        assert!(output.is_empty());
        guard.submitted_and_presented(complete, surface);
        guard.write_diagnostics(&mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.lines().count(), 1);
        assert!(output.contains("event=recovered"));
        assert!(output.contains("primitive=17"));
        assert!(guard.faults.is_empty());
    }

    #[test]
    fn frame_realization_required_fault_surfaces_and_retirement_are_independent() {
        let mut guard = RequiredTextFaultGuard::default();
        let attempt = AppFrameAttempts::new().next();
        let failed = SurfaceId::test_id(361);
        let healthy = SurfaceId::test_id(362);
        guard.prepared(attempt, failed_text(failed, 1, 14));
        guard.prepared(attempt, complete_text(healthy, 1));
        assert!(guard.refused(attempt, failed));
        assert!(!guard.refused(attempt, healthy));
        guard.assert_output_allowed(attempt, healthy);
        guard.submitted_and_presented(attempt, healthy);
        assert_eq!(guard.faults.len(), 1);
        guard.write_diagnostics(&mut Vec::new()).unwrap();
        guard.retire(failed);
        let mut output = Vec::new();
        guard.write_diagnostics(&mut output).unwrap();
        assert!(output.is_empty());
        assert!(guard.faults.is_empty());
        guard.prepared(attempt, failed_text(failed, 1, 14));
        guard.retain_live_surfaces(|surface| surface == healthy);
        assert!(
            guard.faults.is_empty(),
            "retirement through any compositor owner removes custody"
        );
    }

    #[test]
    fn frame_realization_required_fault_healthy_frames_have_no_guard_history() {
        let mut guard = RequiredTextFaultGuard::default();
        let mut attempts = AppFrameAttempts::new();
        let surface = SurfaceId::test_id(361);
        for epoch in 0..1000 {
            let attempt = attempts.next();
            guard.prepared(attempt, complete_text(surface, epoch));
            guard.assert_output_allowed(attempt, surface);
            guard.submitted_and_presented(attempt, surface);
        }
        assert!(guard.faults.is_empty());
        assert_eq!(guard.faults.capacity(), 0);
    }

    #[test]
    #[should_panic(expected = "INV-361-TEXT-FAULT-NO-FALSE-OUTPUT")]
    fn frame_realization_required_fault_contradictory_output_panics_without_observer() {
        let mut guard = RequiredTextFaultGuard::default();
        let attempt = AppFrameAttempts::new().next();
        let surface = SurfaceId::test_id(361);
        guard.prepared(attempt, failed_text(surface, 1, 14));
        guard.assert_output_allowed(attempt, surface);
    }

    #[test]
    #[should_panic(expected = "INV-361-TEXT-FAULT-NO-FALSE-OUTPUT")]
    fn frame_realization_required_fault_contradictory_submit_panics_without_observer() {
        let mut guard = RequiredTextFaultGuard::default();
        let attempt = AppFrameAttempts::new().next();
        let surface = SurfaceId::test_id(361);
        guard.prepared(attempt, failed_text(surface, 1, 14));
        guard.submitted_and_presented(attempt, surface);
    }

    #[test]
    #[should_panic(expected = "INV-361-TEXT-FAULT-NO-FALSE-OUTPUT")]
    fn frame_realization_unprepared_degraded_claim_cannot_produce_output() {
        let mut guard = RequiredTextFaultGuard::default();
        let attempt = AppFrameAttempts::new().next();
        let surface = SurfaceId::test_id(361);
        guard.prepared(
            attempt,
            SurfaceTextRealization::new(
                surface,
                FrameEpoch(1),
                TextRealizationOutcome::Degraded {
                    primitive_count: 1,
                    glyph_count: 2,
                    no_ink_glyph_count: 0,
                    missing_cluster_count: 1,
                    first_failure: TextRealizationFailure {
                        primitive_index: 0,
                        glyph_index: Some(0),
                        reason: crate::renderer::TextRealizationFailureReason::MissingGlyph,
                    },
                },
            ),
        );
        guard.assert_output_allowed(attempt, surface);
    }

    #[test]
    fn frame_realization_opt_in_starts_without_a_receipt() {
        let context = AppContext::new(Theme::default());
        assert!(context.frame_realization_observer().is_none());
        let (_, observer) = AppConfig::default().with_frame_realization_observer();
        assert!(observer.latest().is_none());
        assert!(observer.clone().latest().is_none());
    }

    #[test]
    fn frame_realization_latest_overwrite_preserves_identity_and_immutable_copies() {
        let observer = AppFrameObserver::new();
        let reader = observer.clone();
        let mut attempts = AppFrameAttempts::new();
        let surface = SurfaceId::test_id(361);
        let first = attempts.next();
        observer.publish(AppFrameRecorder::new(first, [surface].into_iter()).finish());
        let held = reader.latest().unwrap();
        let weak = Arc::downgrade(&held);
        let skipped = attempts.next();
        observer.publish(AppFrameRecorder::new(skipped, [surface].into_iter()).finish());
        let latest = attempts.next();
        let mut recorder = AppFrameRecorder::new(latest, [surface].into_iter());
        recorder.unavailable(AppFrameCompileOutcome::LayoutDeferred);
        observer.publish(recorder.finish());
        assert_eq!(held.attempt_id(), first);
        assert_eq!(
            held.compile_outcome(),
            &AppFrameCompileOutcome::SceneUnchanged
        );
        assert_eq!(reader.latest().unwrap().attempt_id(), latest);
        assert_eq!(latest.sequence() - first.sequence(), 2);
        assert_ne!(first, skipped);
        assert_ne!(first, latest);
        let other_run = AppFrameAttempts::new().next();
        assert_eq!(other_run.sequence(), first.sequence());
        assert_ne!(other_run, first);
        drop(held);
        assert!(
            weak.upgrade().is_none(),
            "observer must not retain overwritten history"
        );
    }

    #[test]
    fn frame_realization_unavailable_compile_never_claims_text_or_output() {
        let mut attempts = AppFrameAttempts::new();
        for outcome in [
            AppFrameCompileOutcome::SceneUnchanged,
            AppFrameCompileOutcome::LayoutDeferred,
            AppFrameCompileOutcome::NoTargets,
            AppFrameCompileOutcome::Failed(crate::renderer::RendererSurfaceError::Timeout.into()),
        ] {
            let mut recorder =
                AppFrameRecorder::new(attempts.next(), [SurfaceId::test_id(361)].into_iter());
            recorder.unavailable(outcome.clone());
            let receipt = recorder.finish();
            assert_eq!(receipt.compile_outcome(), &outcome);
            let surface = &receipt.surfaces()[0];
            assert!(surface.text_realization().is_none());
            assert!(!surface.output_produced());
            assert!(!surface.submitted());
            assert!(!surface.presented_called());
        }
    }
}
