    use super::*;
    use wgpu::util::DeviceExt;

    const OLD_DITHER: &str = r#"    // Anti-banding dither: add ±0.5/255 noise to break quantization steps
    // Uses integer hash of screen pixel position — fully deterministic across frames
    let pixel = vec2<u32>(u32(input.position.x), u32(input.position.y));
    var h = pixel.x * 73856093u ^ pixel.y * 19349663u;
    h = (h >> 16u) ^ h;
    let dither_noise = f32(h & 255u) / 255.0;
    let dither = (dither_noise - 0.5) / 255.0;
    // Multiply by alpha to preserve premultiplied invariant
    color = vec4(color.rgb + vec3(dither) * color.a, color.a);
"#;

    fn without_dither(source: &str) -> String {
        let start = source
            .find("    // Anti-banding dither:")
            .expect("dither owner block");
        let end = source[start..]
            .find("    return color * rounded_clip_coverage")
            .map(|offset| start + offset)
            .unwrap();
        format!("{}{}", &source[..start], &source[end..])
    }

    fn old_source() -> String {
        without_dither(GRADIENT_SHADER_SRC).replace(
            "    return color * rounded_clip_coverage",
            &format!("{OLD_DITHER}\n    return color * rounded_clip_coverage"),
        )
    }

    fn params(kind: u32, alpha: f32, clipped: bool) -> GradientParamsUniform {
        let mut result: GradientParamsUniform = bytemuck::Zeroable::zeroed();
        result.rect_size = [256.0, 128.0];
        result.p0 = if kind == 0 { [0.0, 0.0] } else { [0.3, 0.2] };
        result.p1 = if kind == 0 { [1.0, 1.0] } else { [0.9, 0.6] };
        result.kind = kind;
        result.stop_count = 3;
        for (index, rgb) in [[12u8, 17, 23], [16, 19, 24], [20, 23, 29]]
            .iter()
            .enumerate()
        {
            let color = Color::from_srgb(
                f32::from(rgb[0]) / 255.0,
                f32::from(rgb[1]) / 255.0,
                f32::from(rgb[2]) / 255.0,
                1.0,
            );
            result.stops[index] = GradientStopUniform {
                color: [color.r * alpha, color.g * alpha, color.b * alpha, alpha],
                offset: index as f32 * 0.5,
                _pad0: 0.0,
                _pad1: 0.0,
                _pad2: 0.0,
            };
        }
        if clipped {
            result.corner_radius = 12.0;
            result.clip_enabled = 1;
            result.clip = RoundedClipUniform {
                rect_logical: [8.0, 8.0, 240.0, 112.0],
                radii_logical: [16.0, 8.0, 4.0, 12.0],
                inner_min: [16.0, 16.0],
                inner_max: [228.0, 100.0],
                outer_min: [-1.0, -1.0],
                outer_max: [241.0, 113.0],
                aa_radius_logical: 1.0,
                _padding: [0.0; 3],
            };
        }
        result
    }

    fn render(
        gpu: &GpuResources,
        source: &str,
        scale: u32,
        format: wgpu::TextureFormat,
        fields: &[GradientParamsUniform],
        background: wgpu::Color,
    ) -> Vec<[u8; 4]> {
        let width = 256 * scale;
        let height = 128 * scale;
        let globals = Globals {
            view_proj: [
                [2.0 / 256.0, 0.0, 0.0, 0.0],
                [0.0, -2.0 / 128.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0, 1.0],
            ],
            surface_size: [width as f32, height as f32],
            time_seconds: 0.0,
            flags: 1,
        };
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("gradient dither control"),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("gradient dither control"),
                layout: None,
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(GradientVertex::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_main"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent::OVER,
                            alpha: wgpu::BlendComponent::OVER,
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        let buffer = |label, bytes: &[u8], usage| {
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(label),
                    contents: bytes,
                    usage,
                })
        };
        let global_buffer = buffer(
            "gradient globals",
            bytemuck::bytes_of(&globals),
            wgpu::BufferUsages::UNIFORM,
        );
        let globals_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: global_buffer.as_entire_binding(),
            }],
        });
        let field_buffers: Vec<_> = fields
            .iter()
            .map(|field| {
                buffer(
                    "gradient params",
                    bytemuck::bytes_of(field),
                    wgpu::BufferUsages::UNIFORM,
                )
            })
            .collect();
        let groups: Vec<_> = field_buffers
            .iter()
            .map(|field| {
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: None,
                    layout: &pipeline.get_bind_group_layout(1),
                    entries: &[wgpu::BindGroupEntry {
                        binding: 0,
                        resource: field.as_entire_binding(),
                    }],
                })
            })
            .collect();
        let vertices = [[0.0f32, 0.0], [256.0, 0.0], [256.0, 128.0], [0.0, 128.0]];
        let vertex_buffer = buffer(
            "gradient vertices",
            bytemuck::cast_slice(&vertices),
            wgpu::BufferUsages::VERTEX,
        );
        let index_buffer = buffer(
            "gradient indices",
            bytemuck::cast_slice(&[0u16, 1, 2, 0, 2, 3]),
            wgpu::BufferUsages::INDEX,
        );
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gradient readback"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gradient readback"),
            size: u64::from(width * height * 4),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gradient control"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(background),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                multiview_mask: None,
                occlusion_query_set: None,
            });
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &globals_group, &[]);
            pass.set_vertex_buffer(0, vertex_buffer.slice(..));
            pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            for group in &groups {
                pass.set_bind_group(1, group, &[]);
                pass.draw_indexed(0..6, 0, 0..1);
            }
        }
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(width * 4),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        gpu.queue.submit([encoder.finish()]);
        let (send, receive) = std::sync::mpsc::channel();
        readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap()
            });
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        receive.recv().unwrap().unwrap();
        let mapped = readback.slice(..).get_mapped_range().expect("mapped readback range");
        let pixels = mapped
            .chunks_exact(4)
            .map(|pixel| pixel.try_into().unwrap())
            .collect();
        drop(mapped);
        readback.unmap();
        pixels
    }

    #[test]
    fn gradient_dither_dark_surface_metal() {
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real Metal adapter");
        let gpu = renderer.gpu_resources();
        let adapter = gpu.adapter.get_info();
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        println!("gradient adapter: {} {:?}", adapter.name, adapter.backend);
        let no_dither = without_dither(GRADIENT_SHADER_SRC);
        let mut perceptual_bound_holds = true;
        for scale in [1, 2] {
            for kind in [0, 1] {
                let fields = [params(kind, 1.0, false)];
                let baseline = render(
                    &gpu,
                    &no_dither,
                    scale,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    &fields,
                    wgpu::Color::BLACK,
                );
                if kind == 0 {
                    for (pixel, expected) in [
                        (baseline[0], [12u8, 17, 23]),
                        (*baseline.last().unwrap(), [20u8, 23, 29]),
                    ] {
                        assert!(
                            (0..3).all(|channel| pixel[channel].abs_diff(expected[channel]) <= 1),
                            "known linear gradient endpoint changed"
                        );
                    }
                }
                let old = render(
                    &gpu,
                    &old_source(),
                    scale,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    &fields,
                    wgpu::Color::BLACK,
                );
                let current = render(
                    &gpu,
                    GRADIENT_SHADER_SRC,
                    scale,
                    wgpu::TextureFormat::Rgba8UnormSrgb,
                    &fields,
                    wgpu::Color::BLACK,
                );
                let mut old_square = [0.0f64; 3];
                let mut current_square = [0.0f64; 3];
                let mut maximum = [0u8; 3];
                for ((base, old), current) in baseline.iter().zip(&old).zip(&current) {
                    assert_eq!(base[3], current[3]);
                    for channel in 0..3 {
                        old_square[channel] +=
                            f64::from(old[channel].abs_diff(base[channel])).powi(2);
                        let error = current[channel].abs_diff(base[channel]);
                        current_square[channel] += f64::from(error).powi(2);
                        maximum[channel] = maximum[channel].max(error);
                    }
                }
                let rms =
                    |squares: [f64; 3]| squares.map(|sum| (sum / baseline.len() as f64).sqrt());
                let old_rms = rms(old_square);
                let current_rms = rms(current_square);
                println!(
                    "gradient {scale}x kind={kind} old_rms={old_rms:?} current_rms={current_rms:?} maximum={maximum:?}"
                );
                assert!(
                    old_rms[0] > 1.5,
                    "negative control must reproduce amplified linear noise"
                );
                perceptual_bound_holds &= maximum.into_iter().all(|error| error <= 1)
                    && current_rms.into_iter().all(|error| error < 0.65);
            }
        }
        assert!(
            perceptual_bound_holds,
            "display-space dither exceeds one encoded step or its residual bound"
        );
    }

    #[test]
    fn gradient_dither_preserves_premultiplication_alpha_and_rounded_coverage_metal() {
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("real Metal adapter");
        let gpu = renderer.gpu_resources();
        assert_eq!(gpu.adapter.get_info().backend, wgpu::Backend::Metal);
        let no_dither = without_dither(GRADIENT_SHADER_SRC);
        for scale in [1, 2] {
            for alpha in [0.0, 0.25, 1.0] {
                let fields = [params(1, alpha, true)];
                let baseline = render(
                    &gpu,
                    &no_dither,
                    scale,
                    wgpu::TextureFormat::Rgba8Unorm,
                    &fields,
                    wgpu::Color::TRANSPARENT,
                );
                let current = render(
                    &gpu,
                    GRADIENT_SHADER_SRC,
                    scale,
                    wgpu::TextureFormat::Rgba8Unorm,
                    &fields,
                    wgpu::Color::TRANSPARENT,
                );
                for (base, current) in baseline.iter().zip(&current) {
                    assert_eq!(base[3], current[3], "alpha/AA coverage changed");
                    assert!(current[..3].iter().all(|channel| *channel <= current[3]));
                    if current[3] == 0 {
                        assert_eq!(current, &[0; 4]);
                    }
                }
                assert_eq!(current[0], [0; 4], "no parent clip leakage");
            }
            let mut second = params(1, 0.08, false);
            second.p0 = [0.8, 0.6];
            let fields = [params(1, 0.12, false), second];
            let background = wgpu::Color {
                r: 0.003,
                g: 0.005,
                b: 0.008,
                a: 1.0,
            };
            let baseline = render(
                &gpu,
                &no_dither,
                scale,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &fields,
                background,
            );
            let current = render(
                &gpu,
                GRADIENT_SHADER_SRC,
                scale,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &fields,
                background,
            );
            let worst = baseline
                .iter()
                .zip(&current)
                .enumerate()
                .map(|(index, (base, current))| {
                    (
                        (0..3)
                            .map(|channel| base[channel].abs_diff(current[channel]))
                            .max()
                            .unwrap(),
                        index,
                        base,
                        current,
                    )
                })
                .max_by_key(|(error, ..)| *error)
                .unwrap();
            let first_baseline = render(
                &gpu,
                &no_dither,
                scale,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &fields[..1],
                background,
            );
            let first_current = render(
                &gpu,
                GRADIENT_SHADER_SRC,
                scale,
                wgpu::TextureFormat::Rgba8UnormSrgb,
                &fields[..1],
                background,
            );
            assert!(
                first_baseline
                    .iter()
                    .zip(&first_current)
                    .all(|(base, current)| base[3] == current[3]
                        && (0..3).all(|channel| base[channel].abs_diff(current[channel]) <= 1)),
                "one translucent draw exceeds its encoded quantization step"
            );
            let mut square = [0.0f64; 3];
            let mut two_step_pixels = 0;
            for (base, current) in baseline.iter().zip(&current) {
                two_step_pixels +=
                    usize::from((0..3).any(|channel| base[channel].abs_diff(current[channel]) > 1));
                for channel in 0..3 {
                    square[channel] += f64::from(base[channel].abs_diff(current[channel])).powi(2);
                }
            }
            let rms = square.map(|sum| (sum / baseline.len() as f64).sqrt());
            println!(
                "two lobe {scale}x rms={rms:?} two_step_pixels={two_step_pixels}/{} worst={worst:?} first_at_worst={:?}/{:?}",
                baseline.len(),
                first_baseline[worst.1],
                first_current[worst.1]
            );
            // Each blended draw stores and quantizes the sRGB attachment again.
            // Verify the one-draw bound above, and the two stored-draw bound here.
            assert!(
                baseline
                    .iter()
                    .zip(&current)
                    .all(|(base, current)| base[3] == current[3]
                        && (0..3).all(|channel| usize::from(
                            base[channel].abs_diff(current[channel])
                        ) <= fields.len()))
                    && rms.into_iter().all(|error| error < 0.65),
                "translucent lobe composition exceeds its stored-draw error budget: scale={scale} worst={worst:?} rms={rms:?}"
            );
        }
    }
