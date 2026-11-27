//! Private preparation control flow and browser candidate custody.

#[cfg(target_arch = "wasm32")]
use super::Context3DError;
use super::{hdr::HdrError3D, pbr::PbrError3D};

#[cfg(target_arch = "wasm32")]
#[derive(Debug)]
pub(crate) enum PreparationError3D<E> {
    Pending,
    Failed(E),
}

#[cfg(target_arch = "wasm32")]
impl<E> PreparationError3D<E> {
    fn map<F>(self, convert: impl FnOnce(E) -> F) -> PreparationError3D<F> {
        match self {
            Self::Pending => PreparationError3D::Pending,
            Self::Failed(error) => PreparationError3D::Failed(convert(error)),
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) type PbrGpuError3D = PreparationError3D<PbrError3D>;
#[cfg(not(target_arch = "wasm32"))]
pub(super) type PbrGpuError3D = PbrError3D;
#[cfg(target_arch = "wasm32")]
pub(super) type HdrGpuError3D = PreparationError3D<HdrError3D>;
#[cfg(not(target_arch = "wasm32"))]
pub(super) type HdrGpuError3D = HdrError3D;
pub(super) type PbrGpuResult3D<T> = Result<T, PbrGpuError3D>;
pub(super) type HdrGpuResult3D<T> = Result<T, HdrGpuError3D>;

#[cfg(target_arch = "wasm32")]
impl From<PbrError3D> for PbrGpuError3D {
    fn from(error: PbrError3D) -> Self {
        Self::Failed(error)
    }
}
#[cfg(target_arch = "wasm32")]
impl From<Context3DError> for PbrGpuError3D {
    fn from(error: Context3DError) -> Self {
        Self::Failed(error.into())
    }
}
#[cfg(target_arch = "wasm32")]
impl From<HdrError3D> for HdrGpuError3D {
    fn from(error: HdrError3D) -> Self {
        Self::Failed(error)
    }
}
#[cfg(target_arch = "wasm32")]
impl From<Context3DError> for HdrGpuError3D {
    fn from(error: Context3DError) -> Self {
        Self::Failed(error.into())
    }
}

pub(super) fn hdr_refusal(error: PbrError3D) -> HdrError3D {
    match error {
        PbrError3D::Context(error) => HdrError3D::Context(error),
        PbrError3D::Resource(super::context::ControlledResourceError3D::Context(error)) => {
            HdrError3D::Context(error)
        }
        error => HdrError3D::Shader(error.to_string()),
    }
}
fn pbr_refusal(error: HdrError3D) -> PbrError3D {
    match error {
        HdrError3D::Context(error) => PbrError3D::Context(error),
        error => PbrError3D::Shader(error.to_string()),
    }
}
#[cfg(target_arch = "wasm32")]
pub(super) fn pbr_to_hdr(error: impl Into<PbrGpuError3D>) -> HdrGpuError3D {
    error.into().map(hdr_refusal)
}
pub(super) fn hdr_to_pbr(error: impl Into<HdrGpuError3D>) -> PbrGpuError3D {
    #[cfg(target_arch = "wasm32")]
    {
        error.into().map(pbr_refusal)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        pbr_refusal(error.into())
    }
}
pub(super) fn pbr_failure_ref(error: &PbrGpuError3D) -> Option<&PbrError3D> {
    #[cfg(target_arch = "wasm32")]
    {
        match error {
            PreparationError3D::Pending => None,
            PreparationError3D::Failed(error) => Some(error),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(error)
    }
}
pub(super) fn hdr_failure(error: HdrGpuError3D) -> Option<HdrError3D> {
    #[cfg(target_arch = "wasm32")]
    {
        match error {
            PreparationError3D::Pending => None,
            PreparationError3D::Failed(error) => Some(error),
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Some(error)
    }
}

#[cfg(target_arch = "wasm32")]
pub(super) use browser::{CandidateKey3D, CandidatePoll3D, GpuPreparation3D, ScopeCompletion3D};

#[cfg(target_arch = "wasm32")]
mod browser {
    use std::any::Any;
    use std::future::Future;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex};

    use super::{PbrGpuResult3D, PreparationError3D};
    use crate::three_d::Context3D;
    use crate::three_d::context::{DynamicStorageLease3D, PendingRequestLease3D};
    use crate::three_d::pbr::PbrError3D;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub(in crate::three_d) enum CandidateKey3D {
        Program {
            profile: u8,
            sampling: u8,
            object_bytes: u64,
        },
        Pipeline {
            profile: u8,
            sampling: u8,
            object_bytes: u64,
            samples: u32,
            cull: bool,
            observe: bool,
        },
        Shadow {
            profile: u8,
            sampling: u8,
            object_bytes: u64,
            cull: bool,
            cube: bool,
        },
        Material,
        PreparedMaterial,
        Tone(u32),
        Hdr,
        Capacity(bool),
    }

    enum Completion<T> {
        Vacant,
        Pending,
        Ready(T),
        Refused(PbrError3D),
        Taken,
    }

    struct Candidate<T> {
        completion: Mutex<Completion<T>>,
        _metadata: DynamicStorageLease3D,
    }

    pub(in crate::three_d) enum CandidatePoll3D<T> {
        Vacant,
        Ready(T),
    }

    pub(in crate::three_d) enum ScopeCompletion3D<F> {
        One(F),
        Pair(F, F),
    }

    async fn finish_candidate<T, F>(
        scopes: ScopeCompletion3D<F>,
        value: T,
        candidate: Arc<Candidate<T>>,
        owner: Arc<GpuPreparation3D>,
        gpu: crate::renderer::wgpu_renderer::GpuResources,
        request: Arc<PendingRequestLease3D>,
    ) where
        T: Send + Sync + 'static,
        F: Future<Output = Option<wgpu::Error>> + 'static,
    {
        let error = match scopes {
            ScopeCompletion3D::One(scope) => scope.await,
            ScopeCompletion3D::Pair(validation, allocation) => {
                let validation = validation.await;
                let allocation = allocation.await;
                validation.or(allocation)
            }
        };
        if !owner.live.load(Ordering::Acquire) {
            return;
        }
        if !gpu.private_hdr_owner_live() {
            owner.retire();
            return;
        }
        if owner.context_owner.device_failure().is_some() {
            owner.retire();
            owner.revision.fetch_add(1, Ordering::Release);
            gpu.wake_custom_render_preparation();
            return;
        }
        *candidate
            .completion
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = match error {
            Some(error) => Completion::Refused(PbrError3D::Shader(error.to_string())),
            None => Completion::Ready(value),
        };
        owner.revision.fetch_add(1, Ordering::Release);
        gpu.wake_custom_render_preparation();
        drop(request);
    }

    // The function arguments are type witnesses; neither function is invoked.
    // This charges the actual compiler-sized future, including candidate custody,
    // before GPU creation. There is no extra Device operation or adapter probe.
    fn completion_bytes<T, S, F>(
        _push: fn(&wgpu::Device, wgpu::ErrorFilter) -> S,
        _pop: impl FnOnce(S) -> F,
    ) -> usize
    where
        T: Send + Sync + 'static,
        F: Future<Output = Option<wgpu::Error>> + 'static,
    {
        fn bytes<T, F, Task>(
            _finish: fn(
                ScopeCompletion3D<F>,
                T,
                Arc<Candidate<T>>,
                Arc<GpuPreparation3D>,
                crate::renderer::wgpu_renderer::GpuResources,
                Arc<PendingRequestLease3D>,
            ) -> Task,
        ) -> usize {
            std::mem::size_of::<Task>()
        }
        bytes(finish_candidate::<T, F>)
    }

    pub(in crate::three_d) struct GpuPreparation3D {
        live: AtomicBool,
        revision: AtomicU64,
        context_owner: Context3D,
        candidates: Mutex<Vec<(CandidateKey3D, Arc<dyn Any + Send + Sync>)>>,
        request: Mutex<Option<Arc<PendingRequestLease3D>>>,
        _metadata: DynamicStorageLease3D,
    }

    impl GpuPreparation3D {
        pub(in crate::three_d) fn new(
            context: &Context3D,
            request: PendingRequestLease3D,
        ) -> Result<Arc<Self>, PbrError3D> {
            let mut context_owner = context.clone();
            context_owner.gpu_preparation = None;
            Ok(Arc::new(Self {
                live: AtomicBool::new(true),
                revision: AtomicU64::new(0),
                context_owner,
                candidates: Mutex::new(Vec::new()),
                request: Mutex::new(Some(Arc::new(request))),
                _metadata: context.reserve_dynamic_storage(
                    0,
                    0,
                    0,
                    (std::mem::size_of::<Self>()
                        + std::mem::size_of::<PendingRequestLease3D>()
                        + 4 * std::mem::size_of::<usize>()) as u64,
                )?,
            }))
        }

        pub(in crate::three_d) fn retire(&self) {
            self.live.store(false, Ordering::Release);
            self.request
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            self.candidates
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .clear();
        }

        pub(in crate::three_d) fn revision(&self) -> u64 {
            self.revision.load(Ordering::Acquire)
        }

        fn candidate<T: Send + Sync + 'static>(
            &self,
            context: &Context3D,
            key: CandidateKey3D,
        ) -> Result<Arc<Candidate<T>>, PbrError3D> {
            if !self.live.load(Ordering::Acquire)
                || !self.context_owner.same_preparation_owner(context)
                || context.device_failure().is_some()
            {
                return Err(PbrError3D::Association("retired GPU preparation"));
            }
            let mut candidates = self
                .candidates
                .lock()
                .map_err(|_| PbrError3D::Lock("GPU preparation candidates"))?;
            if let Some((_, candidate)) = candidates.iter().find(|(current, _)| *current == key) {
                return candidate
                    .clone()
                    .downcast::<Candidate<T>>()
                    .map_err(|_| PbrError3D::Association("GPU preparation candidate type"));
            }
            let bytes = std::mem::size_of::<Candidate<T>>()
                + std::mem::size_of::<(CandidateKey3D, Arc<dyn Any + Send + Sync>)>()
                + completion_bytes::<T, _, _>(wgpu::Device::push_error_scope, |scope| scope.pop())
                + 4 * std::mem::size_of::<usize>();
            let candidate = Arc::new(Candidate {
                completion: Mutex::new(Completion::Vacant),
                _metadata: context.reserve_dynamic_storage(0, 0, 0, bytes as u64)?,
            });
            let _growth = context.reserve_dynamic_storage(
                0,
                0,
                0,
                (candidates.capacity()
                    * std::mem::size_of::<(CandidateKey3D, Arc<dyn Any + Send + Sync>)>())
                    as u64,
            )?;
            candidates.try_reserve_exact(1).map_err(|_| {
                PbrError3D::Association("GPU preparation candidate storage unavailable")
            })?;
            candidates.push((key, candidate.clone()));
            Ok(candidate)
        }

        pub(in crate::three_d) fn poll<T: Send + Sync + 'static>(
            &self,
            context: &Context3D,
            key: CandidateKey3D,
        ) -> PbrGpuResult3D<CandidatePoll3D<T>> {
            let candidate = self.candidate::<T>(context, key)?;
            let mut completion = candidate
                .completion
                .lock()
                .map_err(|_| PbrError3D::Lock("GPU preparation completion"))?;
            match &*completion {
                Completion::Vacant => Ok(CandidatePoll3D::Vacant),
                Completion::Pending => Err(PreparationError3D::Pending),
                Completion::Refused(error) => Err(PreparationError3D::Failed(error.clone())),
                Completion::Taken => Err(PreparationError3D::Failed(PbrError3D::Association(
                    "GPU candidate already consumed",
                ))),
                Completion::Ready(_) => {
                    match std::mem::replace(&mut *completion, Completion::Taken) {
                        Completion::Ready(value) => Ok(CandidatePoll3D::Ready(value)),
                        _ => unreachable!("completion inspected under the same lock"),
                    }
                }
            }
        }

        pub(in crate::three_d) fn poll_shared<T: Send + Sync + 'static>(
            &self,
            context: &Context3D,
            key: CandidateKey3D,
        ) -> PbrGpuResult3D<CandidatePoll3D<Arc<T>>> {
            let candidate = self.candidate::<Arc<T>>(context, key)?;
            let completion = candidate
                .completion
                .lock()
                .map_err(|_| PbrError3D::Lock("GPU preparation completion"))?;
            match &*completion {
                Completion::Vacant => Ok(CandidatePoll3D::Vacant),
                Completion::Pending => Err(PreparationError3D::Pending),
                Completion::Ready(value) => Ok(CandidatePoll3D::Ready(value.clone())),
                Completion::Refused(error) => Err(PreparationError3D::Failed(error.clone())),
                Completion::Taken => Err(PreparationError3D::Failed(PbrError3D::Association(
                    "GPU candidate already consumed",
                ))),
            }
        }

        pub(in crate::three_d) fn save<T: Send + Sync + 'static>(
            &self,
            context: &Context3D,
            key: CandidateKey3D,
            value: T,
        ) -> Result<(), PbrError3D> {
            let candidate = self.candidate::<T>(context, key)?;
            let mut completion = candidate
                .completion
                .lock()
                .map_err(|_| PbrError3D::Lock("GPU preparation completion"))?;
            if !matches!(*completion, Completion::Vacant | Completion::Taken) {
                return Err(PbrError3D::Association("GPU candidate already retained"));
            }
            *completion = Completion::Ready(value);
            Ok(())
        }

        /// Both error scopes must already have been popped by the creating callback.
        pub(in crate::three_d) fn defer<T, F, R>(
            self: &Arc<Self>,
            context: &Context3D,
            key: CandidateKey3D,
            value: T,
            completion: ScopeCompletion3D<F>,
        ) -> PbrGpuResult3D<R>
        where
            T: Send + Sync + 'static,
            F: Future<Output = Option<wgpu::Error>> + 'static,
        {
            let candidate = self.candidate::<T>(context, key)?;
            {
                let mut state = candidate
                    .completion
                    .lock()
                    .map_err(|_| PbrError3D::Lock("GPU preparation completion"))?;
                if !matches!(*state, Completion::Vacant) {
                    return Err(PreparationError3D::Failed(PbrError3D::Association(
                        "GPU candidate already started",
                    )));
                }
                *state = Completion::Pending;
            }
            // Keep this bounded owner's metadata and Context custody charged
            // until the real scopes finish, even after target retirement.
            // `retire` revokes installation and clears retained ready values.
            let owner = self.clone();
            let gpu = context.gpu_resources()?;
            let request = self
                .request
                .lock()
                .map_err(|_| PbrError3D::Lock("GPU preparation request"))?
                .as_ref()
                .cloned()
                .ok_or(PbrError3D::Association("retired GPU preparation"))?;
            wasm_bindgen_futures::spawn_local(finish_candidate(
                completion,
                value,
                candidate,
                owner,
                gpu,
                request,
            ));
            Err(PreparationError3D::Pending)
        }
    }

    impl Drop for GpuPreparation3D {
        fn drop(&mut self) {
            self.live.store(false, Ordering::Release);
        }
    }
}
