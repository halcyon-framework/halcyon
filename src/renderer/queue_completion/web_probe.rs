#[path = "../queue_completion.rs"]
mod queue_completion;

use halcyon::renderer::image_binding::{ImageAdmissionError, ImageBindingFailure};
use halcyon::renderer::wgpu_renderer::{GpuResources, WgpuRenderer};
use queue_completion::on_submitted_work_done;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;

#[wasm_bindgen(start)]
pub fn install_probe_diagnostics() {
    std::panic::set_hook(Box::new(|info| web_sys::console::error_1(&info.to_string().into())));
}

#[wasm_bindgen(inline_js = r#"
export function pauseProbe() { return new Promise(resolve => setTimeout(resolve, 0)); }
export function injectQueueRejection(queue) {
    const error = new Error("wgpu30 transport rejection fixture");
    const witness = { registrations: 0, errorObserved: false };
    const listen = event => {
        if (event.error === error) {
            witness.errorObserved = true;
            event.preventDefault();
        }
    };
    addEventListener("error", listen);
    witness.stopListening = () => removeEventListener("error", listen);
    witness.restoreQueue = () => delete queue.onSubmittedWorkDone;
    Object.defineProperty(queue, "onSubmittedWorkDone", {
        configurable: true,
        value() {
            witness.registrations += 1;
            return Promise.reject(error);
        },
    });
    return witness;
}
export function delayLossObservation(device, genuineLost) {
    let release;
    const held = new Promise(resolve => { release = resolve; });
    Object.defineProperty(device, "lost", {
        configurable: true,
        value: Promise.all([genuineLost, held]).then(([loss]) => loss),
    });
    return () => {
        delete device.lost;
        release();
    };
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = pauseProbe)]
    fn pause_probe() -> js_sys::Promise;
    #[wasm_bindgen(js_name = injectQueueRejection)]
    fn inject_queue_rejection(queue: &JsValue) -> JsValue;
    #[wasm_bindgen(js_name = delayLossObservation)]
    fn delay_loss_observation(device: &JsValue, genuine_lost: &js_sys::Promise) -> js_sys::Function;
}

struct HeldWork {
    _buffer: wgpu::Buffer,
    drops: Arc<AtomicU32>,
}

impl Drop for HeldWork {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn submit_work(gpu: &GpuResources, drops: &Arc<AtomicU32>) -> HeldWork {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("queue completion witness"),
        size: 64,
        usage: wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.clear_buffer(&buffer, 0, None);
    gpu.queue.submit([encoder.finish()]);
    HeldWork {
        _buffer: buffer,
        drops: drops.clone(),
    }
}

async fn wait_for_probe(predicate: impl Fn() -> bool, pending: &str) -> Result<(), JsValue> {
    for _ in 0..1500 {
        if predicate() {
            return Ok(());
        }
        JsFuture::from(pause_probe()).await?;
    }
    Err(JsValue::from_str(pending))
}

fn property(value: &JsValue, name: &str) -> Result<JsValue, JsValue> {
    js_sys::Reflect::get(value, &JsValue::from_str(name))
}

fn invoke(value: &JsValue, method: &str) -> Result<(), JsValue> {
    property(value, method)?
        .dyn_into::<js_sys::Function>()?
        .call0(value)?;
    Ok(())
}

#[wasm_bindgen]
pub async fn queue_completion_transport_probe(inject_rejection: bool) -> Result<String, JsValue> {
    let renderer = WgpuRenderer::new_headless()
        .await
        .ok_or_else(|| JsValue::from_str("headless WebGPU renderer unavailable"))?;
    let gpu = renderer.gpu_resources();
    let raw = gpu.queue.as_webgpu().ok_or_else(|| JsValue::from_str("WebGPU queue unavailable"))?;
    let calls = Arc::new(AtomicU32::new(0));
    let drops = Arc::new(AtomicU32::new(0));
    let held = submit_work(&gpu, &drops);
    let witness = inject_rejection.then(|| inject_queue_rejection(raw.as_ref()));
    on_submitted_work_done(&gpu.queue, {
        let calls = calls.clone();
        move || {
            calls.fetch_add(1, Ordering::SeqCst);
            drop(held);
        }
    });
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    if let Some(witness) = witness {
        invoke(&witness, "restoreQueue")?;
        assert_eq!(property(&witness, "registrations")?.as_f64(), Some(1.0));
        wait_for_probe(
            || property(&witness, "errorObserved").ok().and_then(|value| value.as_bool()) == Some(true),
            "injected queue rejection was never observed",
        )
        .await?;
        invoke(&witness, "stopListening")?;
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        assert_eq!(drops.load(Ordering::SeqCst), 0);
        Ok(serde_json::json!({
            "case": "injected_rejection",
            "synchronous_registrations": 1,
            "original_error_observed": true,
            "callbacks": 0,
            "drops": 0,
        }).to_string())
    } else {
        wait_for_probe(
            || drops.load(Ordering::SeqCst) == 1,
            "real queue completion did not settle",
        )
        .await?;
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        Ok(serde_json::json!({
            "case": "fulfillment", "callbacks": 1, "drops": 1
        }).to_string())
    }
}

#[wasm_bindgen]
pub async fn queue_completion_device_loss_probe(delay_observer: bool) -> Result<String, JsValue> {
    let mut renderer = WgpuRenderer::new_headless()
        .await
        .ok_or_else(|| JsValue::from_str("headless WebGPU renderer unavailable"))?;
    let gpu = renderer.gpu_resources();
    let raw = gpu.device.as_webgpu().ok_or_else(|| JsValue::from_str("WebGPU device unavailable"))?;
    let genuine_lost = property(raw.as_ref(), "lost")?.dyn_into::<js_sys::Promise>()?;
    let release = delay_observer.then(|| delay_loss_observation(raw.as_ref(), &genuine_lost));

    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("queue completion image-admission witness"),
        size: wgpu::Extent3d { width: 1, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let texture_id = 42;
    renderer.register_image_texture(texture_id, texture);
    let binding = renderer.admit_image(texture_id).expect("initial image admission");
    assert_eq!(binding.availability(), Ok(()));
    let drops = Arc::new(AtomicU32::new(0));
    let held = submit_work(&gpu, &drops);
    let observation = Arc::new(Mutex::new(None));
    on_submitted_work_done(&gpu.queue, {
        let observation = observation.clone();
        let binding = binding.clone();
        move || {
            let mut observation = observation.lock().unwrap();
            assert!(observation.is_none());
            *observation = Some(binding.availability());
            drop(held);
        }
    });
    assert_eq!(drops.load(Ordering::SeqCst), 0);
    gpu.device.destroy();
    wait_for_probe(
        || drops.load(Ordering::SeqCst) == 1,
        "completion Promise unsettled after actual device destruction",
    )
    .await?;
    let lost = JsFuture::from(genuine_lost).await?;
    let loss_reason = property(&lost, "reason")?.as_string().unwrap();
    assert_eq!(loss_reason, "destroyed");
    let at_completion = observation.lock().unwrap().unwrap();
    let loss_seen = at_completion == Err(ImageBindingFailure::DeviceLost);
    if let Some(release) = release {
        assert_eq!(at_completion, Ok(()), "the fixture holds only delivery of real device loss");
        release.call0(&JsValue::UNDEFINED)?;
    }
    wait_for_probe(
        || binding.availability() == Err(ImageBindingFailure::DeviceLost),
        "Halcyon device lifetime observer did not receive the terminal fact",
    )
    .await?;
    assert_eq!(binding.availability(), Err(ImageBindingFailure::DeviceLost));
    assert!(matches!(
        renderer.admit_image(texture_id),
        Err(ImageAdmissionError::Unavailable(ImageBindingFailure::DeviceLost))
    ));
    Ok(serde_json::json!({
        "case": "destroyed_device",
        "forced_observer_delay": delay_observer,
        "browser_loss_reason": loss_reason,
        "completion_observed_after_engine_loss": loss_seen,
        "binding_available_when_callback_ran": at_completion.is_ok(),
        "binding_available_after_observer": false,
        "new_admission_after_observer": "DeviceLost",
        "drops": drops.load(Ordering::SeqCst)
    }).to_string())
}
