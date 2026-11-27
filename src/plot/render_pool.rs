// SPEC-HAL-296 §22.6: PlotRenderPool — Shared worker thread pool.
//
// Adaptive pool size: min(cpus - 2, 4).
// MPMC via crossbeam — any worker renders any Plot.
// Stale job skipping via generation counters.

use crate::plot::market_detail::{MarketDetailLink, MarketDetailReferenceLine};
use crate::plot::price_level::PriceLevel;
use crate::plot::renderer::{PlotRenderer, RenderParams};
use crate::plot::view_state::ViewState;
use crate::renderer::custom_render_pass::CustomRenderPassError;
use crate::renderer::image_binding::ImageAdmissionMailbox;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};

pub(crate) struct CompletedRenderPublication {
    pub generation: u64,
    pub view_state: ViewState,
    pub reference_tag: Option<PriceLevel>,
    pub market_detail_publication: Option<(MarketDetailLink, Option<MarketDetailReferenceLine>)>,
}

pub(crate) struct PresentedPlotLabels {
    pub view_state: ViewState,
    pub reference_tag: Option<PriceLevel>,
}

impl PresentedPlotLabels {
    fn matches(&self, next: &Self) -> bool {
        self.view_state == next.view_state
            && match (&self.reference_tag, &next.reference_tag) {
                (None, None) => true,
                (Some(a), Some(b)) => {
                    a.id == b.id
                        && a.display_y == b.display_y
                        && a.tag_side == b.tag_side
                        && a.tag.len() == b.tag.len()
                        && a.tag.iter().zip(&b.tag).all(|(a, b)| {
                            a.text == b.text && a.color == b.color && a.dimmed == b.dimmed
                        })
                }
                _ => false,
            }
    }
}

impl CompletedRenderPublication {
    pub(crate) fn publish(self, presented_labels: &mut Option<Arc<PresentedPlotLabels>>) {
        let labels = PresentedPlotLabels {
            view_state: self.view_state,
            reference_tag: self.reference_tag,
        };
        if let Some((link, reference)) = self.market_detail_publication {
            link.set_presented_frame(labels.view_state.y_min, labels.view_state.y_max, reference);
        }
        if !presented_labels
            .as_ref()
            .is_some_and(|old| old.matches(&labels))
        {
            *presented_labels = Some(Arc::new(labels));
        }
    }
}

/// A render job submitted by a Plot to the shared pool.
struct RenderJob {
    renderer: Arc<Mutex<PlotRenderer>>,
    params: RenderParams,
    generation: u64,
    series_generation: u64,
    completed_render: Arc<Mutex<Option<CompletedRenderPublication>>>,
    market_detail_publication: Option<(MarketDetailLink, Option<MarketDetailReferenceLine>)>,
}

#[derive(Debug)]
pub(super) enum ResizeRenderFailure {
    PoolClosed,
    WorkerAbandoned,
    RendererPoisoned,
    SourceChanged,
    UnsupportedExtent,
    OutputRefused,
    GenerationExhausted,
    Device(crate::renderer::wgpu_renderer::GpuDeviceFailure),
    Pass(CustomRenderPassError),
}

impl std::fmt::Display for ResizeRenderFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Device(error) => std::fmt::Display::fmt(error, f),
            Self::Pass(error) => std::fmt::Display::fmt(error, f),
            reason => write!(f, "{reason:?}"),
        }
    }
}

pub(super) struct ResizeRenderJob {
    pub params: RenderParams,
    pub width: u32,
    pub height: u32,
    pub generation: u64,
    pub series_data_generation: u64,
    pub current: Arc<AtomicBool>,
    pub market_detail_publication: Option<(MarketDetailLink, Option<MarketDetailReferenceLine>)>,
    pub completed: mpsc::Sender<Result<CompletedRenderPublication, ResizeRenderFailure>>,
    pub completed_ready: Arc<AtomicBool>,
}

struct ResizeRenderExecution<'a> {
    job: Option<ResizeRenderJob>,
    wake: &'a ImageAdmissionMailbox,
}

impl Drop for ResizeRenderExecution<'_> {
    fn drop(&mut self) {
        if let Some(job) = self.job.take() {
            let _ = job
                .completed
                .send(Err(ResizeRenderFailure::WorkerAbandoned));
            job.completed_ready.store(true, Ordering::Release);
        }
        self.wake.notify();
    }
}

/// One latest replacement slot. Its marker travels through the existing pool.
pub(super) struct ResizeRenderSlot {
    renderer: Arc<Mutex<PlotRenderer>>,
    sender: crossbeam_channel::Sender<ResizeRenderJob>,
    receiver: crossbeam_channel::Receiver<ResizeRenderJob>,
    marker: AtomicU8,
    wake: ImageAdmissionMailbox,
}

const RESIZE_IDLE: u8 = 0;
const RESIZE_QUEUED: u8 = 1;
const RESIZE_ABANDONED: u8 = 2;

impl ResizeRenderSlot {
    pub(super) fn new(renderer: Arc<Mutex<PlotRenderer>>, wake: ImageAdmissionMailbox) -> Self {
        let (sender, receiver) = crossbeam_channel::bounded(1);
        Self {
            renderer,
            sender,
            receiver,
            marker: AtomicU8::new(RESIZE_IDLE),
            wake,
        }
    }

    pub(super) fn abandoned(&self) -> bool {
        self.marker.load(Ordering::Acquire) == RESIZE_ABANDONED
    }

    fn abandon_marker(&self) {
        self.marker.store(RESIZE_ABANDONED, Ordering::Release);
        if let Ok(job) = self.receiver.try_recv() {
            drop(ResizeRenderExecution {
                job: Some(job),
                wake: &self.wake,
            });
        }
    }

    fn produce(&self) {
        // Claim the renderer before releasing the marker. While this worker is
        // waiting for an older render, the UI only replaces the latest slot.
        let renderer = self.renderer.lock();
        self.marker.store(RESIZE_IDLE, Ordering::Release);
        let Ok(job) = self.receiver.try_recv() else { return };
        let mut execution = ResizeRenderExecution { job: Some(job), wake: &self.wake };
        let result = match renderer {
            Ok(mut renderer) => Self::render_replacement(
                &mut renderer,
                execution.job.as_ref().expect("claimed Plot replacement"),
            ),
            Err(_) => Err(ResizeRenderFailure::RendererPoisoned),
        };
        let job = execution.job.take().expect("claimed Plot replacement");
        if job.current.load(Ordering::Acquire) {
            let _ = job
                .completed
                .send(result.map(|()| CompletedRenderPublication {
                    generation: job.generation,
                    view_state: job.params.view_state,
                    reference_tag: job.params.reference_tag,
                    market_detail_publication: job.market_detail_publication,
                }));
            job.completed_ready.store(true, Ordering::Release);
        }
    }

    fn render_replacement(
        renderer: &mut PlotRenderer,
        job: &ResizeRenderJob,
    ) -> Result<(), ResizeRenderFailure> {
        if !job.current.load(Ordering::Acquire)
            || renderer.series_data_generation() != job.series_data_generation
            || renderer.last_rendered_generation() >= job.generation
        {
            return Err(ResizeRenderFailure::SourceChanged);
        }
        if let Some(error) = renderer.gpu().observe_device_lifetime().failure() {
            return Err(ResizeRenderFailure::Device(error));
        }
        let limit = renderer.gpu().device.limits().max_texture_dimension_2d;
        if job.width > limit || job.height > limit {
            return Err(ResizeRenderFailure::UnsupportedExtent);
        }
        renderer
            .resize_unpublished(job.width, job.height)
            .map_err(ResizeRenderFailure::Pass)?;
        if !job.current.load(Ordering::Acquire) {
            return Err(ResizeRenderFailure::SourceChanged);
        }
        renderer
            .render(&job.params)
            .map_err(ResizeRenderFailure::Pass)?;
        renderer.set_last_rendered_generation(job.generation);
        Ok(())
    }
}

struct ResizeRenderMarker(Option<Arc<ResizeRenderSlot>>);

impl ResizeRenderMarker {
    fn produce(mut self) {
        self.0.take().expect("queued Plot marker").produce();
    }
}

impl Drop for ResizeRenderMarker {
    fn drop(&mut self) {
        if let Some(slot) = &self.0 {
            slot.abandon_marker();
        }
    }
}

#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
enum RenderWork {
    Frame(RenderJob),
    Resize(ResizeRenderMarker),
}

/// Shared Plot producer. Atomic browser GPU work stays on its calling UI agent.
pub struct PlotRenderPool {
    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    job_tx: crossbeam_channel::Sender<RenderWork>,
    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    _workers: Vec<std::thread::JoinHandle<()>>,
    #[cfg(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    ))]
    closed: bool,
}

impl PlotRenderPool {
    pub fn new() -> Self {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            Self { closed: false }
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            let size = Self::pool_size();
            let (tx, rx) = crossbeam_channel::unbounded();

            let mut workers = Vec::with_capacity(size);
            for i in 0..size {
                let rx = rx.clone();
                workers.push(
                    std::thread::Builder::new()
                        .name(format!("plot-pool-{}", i))
                        .spawn(move || Self::worker_loop(rx))
                        .expect("failed to spawn pool worker"),
                );
            }

            #[cfg(test)]
            crate::renderer::custom_render_pass::audit::record_worker_pool(size);

            Self {
                job_tx: tx,
                _workers: workers,
            }
        }
    }

    pub(crate) fn submit(
        &self,
        renderer: Arc<Mutex<PlotRenderer>>,
        params: RenderParams,
        generation: u64,
        series_generation: u64,
        completed_render: Arc<Mutex<Option<CompletedRenderPublication>>>,
        market_detail_publication: Option<(MarketDetailLink, Option<MarketDetailReferenceLine>)>,
    ) {
        let job = RenderJob {
            renderer,
            params,
            generation,
            series_generation,
            completed_render,
            market_detail_publication,
        };
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        if !self.closed {
            Self::produce_frame(job);
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        let _ = self.job_tx.send(RenderWork::Frame(job));
    }

    pub(super) fn submit_resize(
        &self,
        slot: &Arc<ResizeRenderSlot>,
        job: ResizeRenderJob,
    ) -> Result<(), ResizeRenderFailure> {
        use crossbeam_channel::TrySendError;
        match slot.sender.try_send(job) {
            Ok(()) => {}
            Err(TrySendError::Full(job)) => {
                // Plot is the only sender. Either the worker has already
                // taken the old request or we can displace it here.
                let _ = slot.receiver.try_recv();
                slot.sender
                    .try_send(job)
                    .map_err(|_| ResizeRenderFailure::PoolClosed)?;
            }
            Err(TrySendError::Disconnected(_)) => return Err(ResizeRenderFailure::PoolClosed),
        }
        match slot.marker.compare_exchange(
            RESIZE_IDLE,
            RESIZE_QUEUED,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => {
                let marker = ResizeRenderMarker(Some(slot.clone()));
                #[cfg(all(
                    target_arch = "wasm32",
                    target_os = "unknown",
                    target_feature = "atomics"
                ))]
                {
                    if self.closed {
                        return Err(ResizeRenderFailure::PoolClosed);
                    }
                    marker.produce();
                    Ok(())
                }
                #[cfg(not(all(
                    target_arch = "wasm32",
                    target_os = "unknown",
                    target_feature = "atomics"
                )))]
                self.job_tx
                    .send(RenderWork::Resize(marker))
                    .map_err(|_| ResizeRenderFailure::PoolClosed)
            }
            Err(RESIZE_QUEUED) => Ok(()),
            Err(_) => {
                let _ = slot.receiver.try_recv();
                Err(ResizeRenderFailure::WorkerAbandoned)
            }
        }
    }

    pub(crate) fn resize(
        renderer: &Arc<Mutex<PlotRenderer>>,
        width: u32,
        height: u32,
    ) -> crate::renderer::custom_render_pass::CustomRenderPassResult<()> {
        let mut r = renderer.lock().unwrap();
        r.resize(width, height)
    }

    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    fn pool_size() -> usize {
        let cpus = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4);
        let available = cpus.saturating_sub(2).max(1);
        available.min(4)
    }

    #[cfg(test)]
    pub(crate) fn worker_count_for_test(&self) -> usize {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            0
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            self._workers.len()
        }
    }

    #[cfg(test)]
    pub(crate) fn closed_for_test() -> Self {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            Self { closed: true }
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            let (job_tx, receiver) = crossbeam_channel::unbounded();
            drop(receiver);
            Self {
                job_tx,
                _workers: Vec::new(),
            }
        }
    }

    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    fn worker_loop(rx: crossbeam_channel::Receiver<RenderWork>) {
        while let Ok(first_job) = rx.recv() {
            // Batch-drain: grab everything currently queued.
            // During fast pan, dozens of stale jobs accumulate.
            // Draining is microseconds vs milliseconds of stale renders.
            let mut batch = Vec::with_capacity(32);
            batch.push(first_job);
            while let Ok(job) = rx.try_recv() {
                batch.push(job);
            }

            // Deduplicate by renderer: keep only the highest generation.
            // Use Arc pointer as identity key. Linear scan is fine for
            // ≤20 unique renderers.
            let mut latest: Vec<RenderJob> = Vec::with_capacity(batch.len().min(32));
            for work in batch {
                let job = match work {
                    RenderWork::Frame(job) => job,
                    RenderWork::Resize(marker) => {
                        marker.produce();
                        continue;
                    }
                };
                let ptr = Arc::as_ptr(&job.renderer) as usize;
                if let Some(existing) = latest
                    .iter_mut()
                    .find(|j| Arc::as_ptr(&j.renderer) as usize == ptr)
                {
                    if job.generation > existing.generation {
                        *existing = job;
                    }
                } else {
                    latest.push(job);
                }
            }

            // Render each unique renderer with its latest params
            for job in latest {
                Self::produce_frame(job);
            }
        }
    }

    fn produce_frame(job: RenderJob) {
        let Ok(mut renderer) = job.renderer.lock() else {
            return;
        };
        let wake = renderer.gpu().image_admission.clone();

        // Recheck after acquiring the producer, including on native workers.
        if render_job_is_coherent(
            renderer.last_rendered_generation(),
            renderer.series_generation(),
            job.generation,
            job.series_generation,
            renderer.presentation_pending(),
        ) && renderer.render(&job.params).is_ok()
        {
            renderer.set_last_rendered_generation(job.generation);
            let mut completed = job
                .completed_render
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            *completed = Some(CompletedRenderPublication {
                generation: job.generation,
                view_state: job.params.view_state,
                reference_tag: job.params.reference_tag,
                market_detail_publication: job.market_detail_publication,
            });
        }
        drop(renderer);
        wake.notify();
    }
}

fn render_job_is_coherent(
    last_rendered_generation: u64,
    renderer_series_generation: u64,
    job_generation: u64,
    job_series_generation: u64,
    presentation_pending: bool,
) -> bool {
    job_generation > last_rendered_generation
        && renderer_series_generation == job_series_generation
        && !presentation_pending
}

#[cfg(test)]
pub(crate) fn render_job_is_coherent_for_test(
    last_rendered_generation: u64,
    renderer_series_generation: u64,
    job_generation: u64,
    job_series_generation: u64,
    presentation_pending: bool,
) -> bool {
    render_job_is_coherent(
        last_rendered_generation,
        renderer_series_generation,
        job_generation,
        job_series_generation,
        presentation_pending,
    )
}
