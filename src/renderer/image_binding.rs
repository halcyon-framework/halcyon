use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, AtomicU8, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock, Weak, mpsc};

use super::wgpu_renderer::DeviceLifetimeObservation;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ImageBindingIdentity {
    texture_id: u64,
    generation: NonZeroU64,
}

impl ImageBindingIdentity {
    pub const fn texture_id(self) -> u64 {
        self.texture_id
    }

    pub const fn generation(self) -> NonZeroU64 {
        self.generation
    }

    pub(crate) const fn new(texture_id: u64, generation: NonZeroU64) -> Self {
        Self {
            texture_id,
            generation,
        }
    }
}

pub(crate) fn next_image_generation() -> NonZeroU64 {
    static NEXT: AtomicU64 = AtomicU64::new(1);
    let generation = NEXT
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .expect("Image registration generation exhausted");
    NonZeroU64::new(generation).expect("Image generation is nonzero")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageBindingSampling {
    Linear,
    Nearest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum ImageBindingFailure {
    Unregistered = 1,
    Replaced = 2,
    RendererDestroyed = 3,
    DeviceLost = 4,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageAdmissionError {
    MissingRegistration(u64),
    Unavailable(ImageBindingFailure),
    RequestCancelled,
}

impl std::fmt::Display for ImageAdmissionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Image admission failed: {self:?}")
    }
}

impl std::error::Error for ImageAdmissionError {}

/// CPU evidence for one actual renderer registration. This owns no GPU handle
/// and neither extends resource lifetime nor authorizes a replacement generation.
#[derive(Clone)]
pub struct ImageBinding {
    pub(crate) state: Arc<ImageBindingState>,
}

impl ImageBinding {
    pub fn identity(&self) -> ImageBindingIdentity {
        self.state.identity
    }

    pub fn sampling(&self) -> ImageBindingSampling {
        self.state.sampling
    }

    pub fn availability(&self) -> Result<(), ImageBindingFailure> {
        self.state.availability()
    }

    pub(crate) fn observe(&self, queue: &Arc<ImageBindingInvalidations>) {
        let mut observers = self
            .state
            .observers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        observers.retain(|observer| observer.strong_count() != 0);
        let newly_observed = !observers
            .iter()
            .any(|observer| observer.ptr_eq(&Arc::downgrade(queue)));
        if newly_observed {
            observers.push(Arc::downgrade(queue));
        }
        if newly_observed && self.state.revoked.load(Ordering::Acquire) != 0 {
            queue.push(self.identity());
        }
    }
}

impl std::fmt::Debug for ImageBinding {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ImageBinding")
            .field("identity", &self.identity())
            .field("sampling", &self.sampling())
            .finish()
    }
}

impl PartialEq for ImageBinding {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for ImageBinding {}

pub(crate) struct ImageBindingState {
    identity: ImageBindingIdentity,
    sampling: ImageBindingSampling,
    device: DeviceLifetimeObservation,
    revoked: AtomicU8,
    observers: Mutex<Vec<Weak<ImageBindingInvalidations>>>,
}

impl ImageBindingState {
    pub(crate) fn is_revoked(&self) -> bool {
        self.revoked.load(Ordering::Acquire) != 0
    }

    pub(crate) fn new(
        identity: ImageBindingIdentity,
        sampling: ImageBindingSampling,
        device: DeviceLifetimeObservation,
    ) -> Arc<Self> {
        let state = Arc::new(Self {
            identity,
            sampling,
            device: device.clone(),
            revoked: AtomicU8::new(0),
            observers: Mutex::new(Vec::new()),
        });
        device.observe_image_binding(&state);
        state
    }

    fn availability(&self) -> Result<(), ImageBindingFailure> {
        match self.revoked.load(Ordering::Acquire) {
            1 => Err(ImageBindingFailure::Unregistered),
            2 => Err(ImageBindingFailure::Replaced),
            3 => Err(ImageBindingFailure::RendererDestroyed),
            4 => Err(ImageBindingFailure::DeviceLost),
            _ if self.device.failure().is_some() => Err(ImageBindingFailure::DeviceLost),
            _ => Ok(()),
        }
    }

    pub(crate) fn revoke(&self, reason: ImageBindingFailure) {
        if self
            .revoked
            .compare_exchange(0, reason as u8, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        let mut observers = self
            .observers
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        observers.retain(|observer| {
            if let Some(queue) = observer.upgrade() {
                queue.push(self.identity);
                true
            } else {
                false
            }
        });
    }
}

#[derive(Default)]
pub(crate) struct ImageBindingInvalidations {
    pending: Mutex<Vec<ImageBindingIdentity>>,
    wake: Option<Box<super::gpu_custody::GpuNotify>>,
}

impl ImageBindingInvalidations {
    pub(crate) fn new(wake: Option<Box<super::gpu_custody::GpuNotify>>) -> Self {
        Self {
            pending: Mutex::new(Vec::new()),
            wake,
        }
    }

    fn push(&self, identity: ImageBindingIdentity) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let newly_pending = !pending.contains(&identity);
        if newly_pending {
            pending.push(identity);
        }
        drop(pending);
        if newly_pending {
            if let Some(wake) = &self.wake {
                wake();
            }
        }
    }

    pub(crate) fn take(&self) -> Vec<ImageBindingIdentity> {
        std::mem::take(
            &mut *self
                .pending
                .lock()
                .unwrap_or_else(|error| error.into_inner()),
        )
    }
}

pub struct ImageBindingRequest {
    receiver: mpsc::Receiver<Result<ImageBinding, ImageAdmissionError>>,
    result: OnceLock<Result<ImageBinding, ImageAdmissionError>>,
}

impl ImageBindingRequest {
    pub(crate) fn pending(texture_id: u64) -> (Self, PendingImageAdmission) {
        let (sender, receiver) = mpsc::channel();
        (
            Self {
                receiver,
                result: OnceLock::new(),
            },
            PendingImageAdmission {
                texture_id,
                sender,
                expected_view: None,
                _retained: None,
                plot_front: None,
            },
        )
    }

    pub fn try_resolve(&self) -> Result<Option<ImageBinding>, ImageAdmissionError> {
        if let Some(result) = self.result.get() {
            return result.clone().map(Some);
        }
        match self.receiver.try_recv() {
            Ok(result) => {
                let _ = self.result.set(result);
                self.result
                    .get()
                    .expect("received Image admission result")
                    .clone()
                    .map(Some)
            }
            Err(mpsc::TryRecvError::Empty) => Ok(None),
            Err(mpsc::TryRecvError::Disconnected) => Err(ImageAdmissionError::RequestCancelled),
        }
    }
}

#[derive(Clone, Default)]
pub struct ImageAdmissionMailbox {
    state: Arc<ImageAdmissionMailboxState>,
}

#[derive(Default)]
struct ImageAdmissionMailboxState {
    pending: Mutex<PendingImageAdmissions>,
    has_items: AtomicBool,
    wake: OnceLock<Box<super::gpu_custody::GpuNotify>>,
}

#[derive(Default)]
struct PendingImageAdmissions {
    items: Option<Vec<PendingImageAdmission>>,
    closed: bool,
}

pub(crate) struct PendingImageAdmission {
    pub(crate) texture_id: u64,
    pub(crate) sender: mpsc::Sender<Result<ImageBinding, ImageAdmissionError>>,
    // HAL381: only the private resource owner may request evidence for a
    // particular retained view. An equal public texture ID is insufficient.
    pub(crate) expected_view: Option<wgpu::TextureView>,
    // A pending exact-view request itself owns a physical view. Keep its real
    // storage charge alive even if the caller cancels before renderer drain.
    _retained: Option<Arc<super::gpu_custody::GpuCharge>>,
    pub(crate) plot_front: Option<(
        super::wgpu_renderer::PendingImageRegistration,
        Arc<AtomicBool>,
    )>,
}

impl ImageAdmissionMailbox {
    pub(crate) fn set_wake(&self, wake: Box<super::gpu_custody::GpuNotify>) {
        assert!(
            self.state.wake.set(wake).is_ok(),
            "Image admission wake owner already configured"
        );
    }

    pub(crate) fn notify(&self) {
        if let Some(wake) = self.state.wake.get() {
            wake();
        }
    }

    pub(crate) fn has_pending(&self) -> bool {
        self.state.has_items.load(Ordering::Acquire)
    }

    pub(crate) fn request(&self, texture_id: u64) -> ImageBindingRequest {
        self.request_exact_view(texture_id, None, None)
    }

    pub(crate) fn request_exact_view(
        &self,
        texture_id: u64,
        expected_view: Option<wgpu::TextureView>,
        retained: Option<Arc<super::gpu_custody::GpuCharge>>,
    ) -> ImageBindingRequest {
        let (request, mut admission) = ImageBindingRequest::pending(texture_id);
        admission.expected_view = expected_view;
        admission._retained = retained;
        self.enqueue(admission);
        request
    }

    /// Only the Plot pass owner can supply the submitted view and its live request.
    pub(crate) fn request_plot_front(
        &self,
        registration: super::wgpu_renderer::PendingImageRegistration,
        current: Arc<AtomicBool>,
    ) -> ImageBindingRequest {
        let (request, mut admission) = ImageBindingRequest::pending(registration.texture_id);
        admission.expected_view = Some(registration.view.clone());
        admission.plot_front = Some((registration, current));
        self.enqueue(admission);
        request
    }

    fn enqueue(&self, admission: PendingImageAdmission) {
        let mut pending = self
            .state
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if !pending.closed {
            let items = pending.items.get_or_insert_with(Vec::new);
            let newly_pending = items.is_empty();
            if admission.plot_front.is_some() {
                items.retain(|item| {
                    item.plot_front.as_ref().is_none_or(|(_, current)| {
                        current.load(Ordering::Acquire)
                    })
                });
            }
            items.push(admission);
            self.state.has_items.store(true, Ordering::Release);
            drop(pending);
            if newly_pending {
                self.notify();
            }
        }
    }

    pub(crate) fn close(&self) {
        let mut pending = self
            .state
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        pending.closed = true;
        let requests = pending.items.take().unwrap_or_default();
        self.state.has_items.store(false, Ordering::Release);
        let had_requests = !requests.is_empty();
        drop(pending);
        drop(requests);
        if had_requests {
            self.notify();
        }
    }

    pub(crate) fn take(&self) -> Vec<PendingImageAdmission> {
        if !self.has_pending() {
            return Vec::new();
        }
        let mut pending = self
            .state
            .pending
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let requests = pending.items.take().unwrap_or_default();
        self.state.has_items.store(false, Ordering::Release);
        requests
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal381_cancelled_exact_view_admission_keeps_retention_until_drain_or_close() {
        let mailbox = ImageAdmissionMailbox::default();
        let charge = Arc::new(());
        let weak = Arc::downgrade(&charge);
        let request = mailbox.request_exact_view(381, None, Some(charge));
        drop(request);
        assert!(
            weak.upgrade().is_some(),
            "a pending physical-view observation still owns its charge"
        );
        let pending = mailbox.take();
        assert_eq!(pending.len(), 1);
        assert!(weak.upgrade().is_some());
        drop(pending);
        assert!(weak.upgrade().is_none());
        let charge = Arc::new(());
        let weak = Arc::downgrade(&charge);
        let request = mailbox.request_exact_view(382, None, Some(charge));
        mailbox.close();
        assert!(weak.upgrade().is_none());
        assert_eq!(
            request.try_resolve(),
            Err(ImageAdmissionError::RequestCancelled)
        );
    }

    #[test]
    fn image_admission_unused_mailbox_retirement_allocates_no_pending_queue() {
        let mailbox = ImageAdmissionMailbox::default();
        assert!(mailbox.take().is_empty());
        assert!(mailbox.state.pending.lock().unwrap().items.is_none());
        mailbox.close();
        assert!(mailbox.take().is_empty());
        assert!(mailbox.state.pending.lock().unwrap().items.is_none());
        assert!(!mailbox.has_pending());
    }

    #[test]
    fn image_admission_mailbox_retirement_serializes_concurrent_requests() {
        let mailbox = ImageAdmissionMailbox::default();
        let pending = mailbox.request(1);
        let barrier = Arc::new(std::sync::Barrier::new(2));
        let producer = mailbox.clone();
        let producer_barrier = barrier.clone();
        let worker = std::thread::spawn(move || {
            producer_barrier.wait();
            (0..64)
                .map(|texture_id| producer.request(texture_id))
                .collect::<Vec<_>>()
        });
        barrier.wait();
        mailbox.close();
        assert_eq!(
            pending.try_resolve(),
            Err(ImageAdmissionError::RequestCancelled)
        );
        for request in worker.join().unwrap() {
            assert_eq!(
                request.try_resolve(),
                Err(ImageAdmissionError::RequestCancelled)
            );
        }
        assert!(!mailbox.has_pending());
    }
}
