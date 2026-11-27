//! Bounds for GPU custody on the adopted shared-memory browser profile.
//! CPU jobs and their proof values retain their own Send contracts.

#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
pub(crate) trait GpuValue: std::any::Any {}
#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
impl<T: std::any::Any + ?Sized> GpuValue for T {}

#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
pub(crate) trait GpuValue: std::any::Any + Send + Sync {}
#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
impl<T: std::any::Any + Send + Sync + ?Sized> GpuValue for T {}

/// Keeps the original typed Arc allocation; only its GPU-local erasure differs.
pub(crate) struct GpuErasedArc {
    #[cfg(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    ))]
    value: Box<dyn std::any::Any>,
    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    value: std::sync::Arc<dyn std::any::Any + Send + Sync>,
}

impl GpuErasedArc {
    pub(crate) fn new<T: GpuValue>(value: std::sync::Arc<T>) -> Self {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            Self {
                value: Box::new(value),
            }
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            Self { value }
        }
    }

    pub(crate) fn clone_as<T: GpuValue>(&self) -> Option<std::sync::Arc<T>> {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            self.value
                .as_ref()
                .downcast_ref::<std::sync::Arc<T>>()
                .cloned()
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            self.value.clone().downcast::<T>().ok()
        }
    }

    pub(crate) const fn metadata_bytes() -> usize {
        #[cfg(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        ))]
        {
            std::mem::size_of::<std::sync::Arc<()>>()
        }
        #[cfg(not(all(
            target_arch = "wasm32",
            target_os = "unknown",
            target_feature = "atomics"
        )))]
        {
            0
        }
    }
}

#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
pub(crate) trait GpuCompletionCallback: 'static {}
#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
impl<T: 'static> GpuCompletionCallback for T {}

#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
pub(crate) trait GpuCompletionCallback: Send + 'static {}
#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
impl<T: Send + 'static> GpuCompletionCallback for T {}

#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
pub(crate) type GpuNotify = dyn Fn();
#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
pub(crate) type GpuNotify = dyn Fn() + Send + Sync;

#[cfg(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
))]
pub(crate) type GpuCharge = dyn std::any::Any;
#[cfg(not(all(
    target_arch = "wasm32",
    target_os = "unknown",
    target_feature = "atomics"
)))]
pub(crate) type GpuCharge = dyn Send + Sync;

#[cfg(test)]
mod tests {
    use super::GpuErasedArc;
    use std::sync::Arc;

    #[test]
    fn gpu_erasure_preserves_the_exact_typed_allocation() {
        let original = Arc::new(vec![1_u8, 2, 3]);
        let erased = GpuErasedArc::new(original.clone());
        let recovered = erased.clone_as::<Vec<u8>>().unwrap();
        assert!(Arc::ptr_eq(&original, &recovered));
        assert!(erased.clone_as::<Vec<u16>>().is_none());
        drop(erased);
        assert_eq!(Arc::strong_count(&original), 2);
    }

    #[test]
    fn cpu_proof_payloads_keep_their_worker_contract() {
        use crate::three_d::*;
        fn worker<T: Send + Sync>() {}
        worker::<PbrNormalCpuProofPrepared3D>();
        worker::<PbrNormalCpuProofJob3D>();
        worker::<PbrNormalCpuProofResult3D>();
        worker::<PbrGeometryCpuPrepared3D>();
        worker::<PbrGeometryCpuSuccessorJob3D>();
        worker::<PbrGeometryCpuResult3D>();
    }

    #[cfg(not(all(
        target_arch = "wasm32",
        target_os = "unknown",
        target_feature = "atomics"
    )))]
    #[test]
    fn existing_profiles_keep_their_gpu_thread_contracts() {
        fn threaded<T: Send + Sync + ?Sized>() {}
        threaded::<crate::renderer::wgpu_renderer::GpuResources>();
        threaded::<crate::three_d::Context3D>();
        threaded::<crate::renderer::CustomRenderPass>();
        threaded::<dyn crate::plot::Series>();
        threaded::<dyn crate::renderer::render_graph::PassExecutor>();
    }
}
