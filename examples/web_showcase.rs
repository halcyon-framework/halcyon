//! The canonical ShowcaseScreen through Halcyon's ordinary browser App path.

#[allow(dead_code)]
#[path = "showcase/main.rs"]
mod showcase;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn browser_start() {
    if let Err(error) = tracing_subscriber::fmt()
        .with_ansi(false)
        .without_time()
        .with_target(false)
        .with_max_level(tracing::Level::ERROR)
        .with_writer(|| BrowserErrors(String::new()))
        .try_init()
    {
        report_error(&error.to_string());
    }
    std::panic::set_hook(Box::new(|info| report_error(&info.to_string())));
    showcase::run_web_showcase(report_gpu_init_error);
}

#[cfg(target_arch = "wasm32")]
struct BrowserErrors(String);

#[cfg(target_arch = "wasm32")]
impl std::io::Write for BrowserErrors {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.push_str(&String::from_utf8_lossy(bytes));
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(target_arch = "wasm32")]
impl Drop for BrowserErrors {
    fn drop(&mut self) {
        if !self.0.is_empty() {
            report_error(&self.0);
        }
    }
}

#[cfg(target_arch = "wasm32")]
fn report_error(message: &str) {
    web_sys::console::error_1(&message.into());
    show_error(message, false);
}

#[cfg(target_arch = "wasm32")]
fn report_gpu_init_error(error: halcyon::renderer::wgpu_renderer::RendererInitError) {
    use halcyon::renderer::wgpu_renderer::RendererInitError;

    let message = match &error {
        RendererInitError::AdapterUnavailable(_) => {
            "No compatible WebGPU adapter is available. Check your browser's WebGPU support and hardware acceleration.".to_owned()
        }
        _ => error.to_string(),
    };
    web_sys::console::error_1(&format!("Halcyon GPU initialization failed: {error:?}").into());
    show_error(&message, true);
}

#[cfg(target_arch = "wasm32")]
fn show_error(message: &str, replace_message: bool) {
    if let Some(output) = web_sys::window()
        .and_then(|window| window.document())
        .and_then(|document| document.get_element_by_id("boot-error"))
    {
        if replace_message || output.text_content().is_none_or(|text| text.is_empty()) {
            output.set_text_content(Some(message));
        }
        output.remove_attribute("hidden").unwrap_or_else(|error| {
            web_sys::console::error_1(&error);
        });
    }
}
