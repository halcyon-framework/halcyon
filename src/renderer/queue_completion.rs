/// Register completion on the queue that accepted the renderer's submission.
pub(crate) fn on_submitted_work_done(queue: &wgpu::Queue, callback: impl FnOnce() + Send + 'static) {
    #[cfg(all(target_arch = "wasm32", target_os = "unknown"))]
    if let Some(queue) = queue.as_webgpu() {
        let completion = queue.on_submitted_work_done();
        wasm_bindgen_futures::spawn_local(async move {
            match wasm_bindgen_futures::JsFuture::from(completion).await {
                Ok(_) => callback(),
                Err(error) => {
                    // A rejected transport leaves completion uncertain. Retain
                    // the callback's captured custody as in the original backend.
                    core::mem::forget(callback);
                    tracing::error!("WebGPU queue completion was not confirmed: {error:?}");
                    wasm_bindgen::throw_val(error);
                }
            }
        });
        return;
    }

    queue.on_submitted_work_done(callback);
}
