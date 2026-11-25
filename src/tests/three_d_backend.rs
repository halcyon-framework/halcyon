#[cfg(target_os = "macos")]
#[test]
fn metal_backend_conformance_gate_requires_an_actual_default_profile_device() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::METAL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        compatible_surface: None,
        apply_limit_buckets: false,
        force_fallback_adapter: false,
    }))
    .expect("SPEC-HAL-355 Metal conformance cannot pass when no Metal adapter is available");
    let info = adapter.get_info();
    assert_eq!(info.backend, wgpu::Backend::Metal);

    let (device, _queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: wgpu::Features::empty(),
        required_limits: wgpu::Limits::default(),
        experimental_features: wgpu::ExperimentalFeatures::disabled(),
        memory_hints: Default::default(),
        label: Some("SPEC-HAL-355 Metal Conformance Gate"),
        trace: Default::default(),
    }))
    .expect("SPEC-HAL-355 Metal conformance requires the exact baseline device profile");
    assert_eq!(device.features(), wgpu::Features::empty());

    println!(
        "SPEC-HAL-355 Metal adapter: name={:?}, vendor={:#x}, device={:#x}, type={:?}, driver={:?}, driver_info={:?}, backend={:?}",
        info.name,
        info.vendor,
        info.device,
        info.device_type,
        info.driver,
        info.driver_info,
        info.backend,
    );
}
