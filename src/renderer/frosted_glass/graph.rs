use super::{FrostedGlassCompileRefusal as Refusal, Occurrence};
use crate::renderer::{
    bloom::{BloomPlan, BloomStage, HDR_FORMAT},
    compiled_draw_plan::SurfaceDrawCommand,
    render_graph::{
        AttachmentDesc, AttachmentHandle, AttachmentSize, BufferDesc, BufferHandle, GraphError,
        PassContext, PassDrawScope, PassExecutionError, PassExecutor, RenderGraphBuilder,
        RenderGraphTopology, RenderPassNode, ResourceRef,
    },
};
use std::{collections::HashMap, ops::Range};

const SEGMENTS: [&str; 19] = [
    "frost_segment_0",
    "frost_segment_1",
    "frost_segment_2",
    "frost_segment_3",
    "frost_segment_4",
    "frost_segment_5",
    "frost_segment_6",
    "frost_segment_7",
    "frost_segment_8",
    "frost_segment_9",
    "frost_segment_10",
    "frost_segment_11",
    "frost_segment_12",
    "frost_segment_13",
    "frost_segment_14",
    "frost_segment_15",
    "frost_segment_16",
    "frost_segment_17",
    "frost_segment_18",
];
const COPIES: [&str; 16] = [
    "frost_copy_0",
    "frost_copy_1",
    "frost_copy_2",
    "frost_copy_3",
    "frost_copy_4",
    "frost_copy_5",
    "frost_copy_6",
    "frost_copy_7",
    "frost_copy_8",
    "frost_copy_9",
    "frost_copy_10",
    "frost_copy_11",
    "frost_copy_12",
    "frost_copy_13",
    "frost_copy_14",
    "frost_copy_15",
];
const HORIZONTAL: [&str; 16] = [
    "frost_horizontal_0",
    "frost_horizontal_1",
    "frost_horizontal_2",
    "frost_horizontal_3",
    "frost_horizontal_4",
    "frost_horizontal_5",
    "frost_horizontal_6",
    "frost_horizontal_7",
    "frost_horizontal_8",
    "frost_horizontal_9",
    "frost_horizontal_10",
    "frost_horizontal_11",
    "frost_horizontal_12",
    "frost_horizontal_13",
    "frost_horizontal_14",
    "frost_horizontal_15",
];
const VERTICAL: [&str; 16] = [
    "frost_vertical_0",
    "frost_vertical_1",
    "frost_vertical_2",
    "frost_vertical_3",
    "frost_vertical_4",
    "frost_vertical_5",
    "frost_vertical_6",
    "frost_vertical_7",
    "frost_vertical_8",
    "frost_vertical_9",
    "frost_vertical_10",
    "frost_vertical_11",
    "frost_vertical_12",
    "frost_vertical_13",
    "frost_vertical_14",
    "frost_vertical_15",
];
const REPLACE: [&str; 16] = [
    "frost_replace_0",
    "frost_replace_1",
    "frost_replace_2",
    "frost_replace_3",
    "frost_replace_4",
    "frost_replace_5",
    "frost_replace_6",
    "frost_replace_7",
    "frost_replace_8",
    "frost_replace_9",
    "frost_replace_10",
    "frost_replace_11",
    "frost_replace_12",
    "frost_replace_13",
    "frost_replace_14",
    "frost_replace_15",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PhysicalRole {
    Accumulator,
    Snapshot(usize),
    Horizontal(usize),
    Filtered(usize),
    Bloom(usize),
    Terminal,
}
#[derive(Debug, Clone, Copy)]
pub(super) enum Filter {
    Horizontal,
    Vertical,
    Replace,
    Terminal,
    Bloom(BloomStage),
}

/// CPU only. Each accumulator version has a distinct single writer; the full
/// explicit chain proves all old-version readers precede physical overwrite.
pub(crate) struct FrostedSurfacePlan {
    pub topology: RenderGraphTopology,
    pub output: AttachmentHandle,
    pub occurrences: Vec<Occurrence>,
    pub(crate) bloom: Option<BloomPlan>,
    pub(super) roles: HashMap<AttachmentHandle, PhysicalRole>,
    pub(super) groups: Vec<(BufferHandle, Filter, usize)>,
    pub size: (u32, u32),
    pub format: wgpu::TextureFormat,
    pub bytes: u64,
}

fn attachment(
    builder: &mut RenderGraphBuilder,
    format: wgpu::TextureFormat,
    size: (u32, u32),
) -> AttachmentHandle {
    builder.declare_attachment(AttachmentDesc {
        label: "HAL385 logical attachment version",
        size: AttachmentSize::Absolute {
            width: size.0,
            height: size.1,
        },
        format,
        samples: 1,
        clear_color: None,
        load_op: wgpu::LoadOp::Load,
        store_op: wgpu::StoreOp::Store,
    })
}
fn uniform(builder: &mut RenderGraphBuilder) -> BufferHandle {
    builder.declare_buffer(BufferDesc {
        label: "HAL385 immutable occurrence",
        size: std::mem::size_of::<super::FrostedGlassParams>() as u64,
        usage: wgpu::BufferUsages::UNIFORM,
    })
}
impl FrostedSurfacePlan {
    pub fn new<P: crate::compositor::Contract>(
        draw_plan: &crate::renderer::draw_plan::DrawPlan<P>,
        commands: &[SurfaceDrawCommand],
        occurrences: Vec<Occurrence>,
        bloom: Option<BloomPlan>,
        size: (u32, u32),
        format: wgpu::TextureFormat,
    ) -> Result<Self, Refusal> {
        let mut bytes = u64::from(size.0)
            .checked_mul(u64::from(size.1))
            .and_then(|v| v.checked_mul(4))
            .and_then(|v| v.checked_add(std::mem::size_of::<super::FrostedGlassParams>() as u64))
            .ok_or(Refusal::ResourceBudget {
                required_bytes: u64::MAX,
                limit_bytes: super::PLAN_BUDGET,
            })?;
        for o in &occurrences {
            bytes = bytes
                .checked_add(o.params.bytes().ok_or(Refusal::ResourceBudget {
                    required_bytes: u64::MAX,
                    limit_bytes: super::PLAN_BUDGET,
                })?)
                .ok_or(Refusal::ResourceBudget {
                    required_bytes: u64::MAX,
                    limit_bytes: super::PLAN_BUDGET,
                })?;
        }
        if let Some(bloom) = &bloom {
            bytes = bloom.bytes().and_then(|charge| bytes.checked_add(charge)).ok_or(
                Refusal::ResourceBudget { required_bytes: u64::MAX, limit_bytes: super::PLAN_BUDGET },
            )?;
        }
        let mut b = RenderGraphBuilder::new();
        let mut roles = HashMap::new();
        let mut groups = Vec::new();
        let mut previous = None;
        let mut version = None;
        let mut segment = 0usize;
        for scope in [PassDrawScope::MainUnderlay, PassDrawScope::Overlay] {
            let mut start = 0;
            let markers: Vec<_> = commands
                .iter()
                .enumerate()
                .filter_map(|(i, c)| {
                    if !scope.contains_z(c.z_index()) {
                        return None;
                    }
                    let frost = match c {
                        SurfaceDrawCommand::FrostedGlass { ordinal, .. } => {
                            occurrences.iter().position(|o| o.ordinal == *ordinal)
                        }
                        _ => None,
                    };
                    let blooms = bloom.as_ref().is_some_and(|b| b.command == i);
                    (frost.is_some() || blooms).then_some((i, frost, blooms))
                })
                .collect();
            for (at, index, blooms) in markers
                .iter()
                .copied()
                .chain(std::iter::once((commands.len(), None, false)))
            {
                // An analytic source is drawn by the normal range. A Frosted
                // source is instead completed by its replacement below.
                let end = at + usize::from(blooms && index.is_none());
                let out = attachment(&mut b, format, size);
                roles.insert(out, PhysicalRole::Accumulator);
                let first = version.is_none();
                let mut pass = RenderPassNode::new(
                    SEGMENTS[segment],
                    Box::new(NormalSegment {
                        output: out,
                        range: start..end,
                        scope,
                        clear: first,
                    }),
                )
                .write(ResourceRef::Attachment(out));
                if let Some(old) = version {
                    pass = pass.read(ResourceRef::Attachment(old));
                }
                if let Some(dep) = previous {
                    pass = pass.depends_on(dep);
                }
                previous = Some(b.add_pass(pass));
                version = Some(out);
                segment += 1;
                if let Some(n) = index {
                let p = &occurrences[n].params;
                let s = (p.snapshot[2], p.snapshot[3]);
                let v = (p.filtered_domain[2], p.filtered_domain[3]);
                let snapshot = attachment(&mut b, format, s);
                roles.insert(snapshot, PhysicalRole::Snapshot(n));
                let hx = attachment(&mut b, wgpu::TextureFormat::Rgba16Float, s);
                roles.insert(hx, PhysicalRole::Horizontal(n));
                let filtered = attachment(&mut b, wgpu::TextureFormat::Rgba16Float, v);
                roles.insert(filtered, PhysicalRole::Filtered(n));
                let copy = b.add_pass(
                    RenderPassNode::new(
                        COPIES[n],
                        Box::new(CopySnapshot {
                            source: out,
                            destination: snapshot,
                            roi: p.snapshot,
                        }),
                    )
                    .read(ResourceRef::Attachment(out))
                    .write(ResourceRef::Attachment(snapshot))
                    .depends_on(previous.unwrap()),
                );
                let hgroup = uniform(&mut b);
                groups.push((hgroup, Filter::Horizontal, n));
                let horizontal = b.add_pass(
                    RenderPassNode::new(
                        HORIZONTAL[n],
                        Box::new(FilterPass {
                            output: hx,
                            group: hgroup,
                            filter: Filter::Horizontal,
                            viewport: [0, 0, s.0, s.1],
                            scissor: None,
                        }),
                    )
                    .read(ResourceRef::Attachment(snapshot))
                    .read(ResourceRef::Buffer(hgroup))
                    .write(ResourceRef::Attachment(hx))
                    .depends_on(copy),
                );
                let vgroup = uniform(&mut b);
                groups.push((vgroup, Filter::Vertical, n));
                let vertical = b.add_pass(
                    RenderPassNode::new(
                        VERTICAL[n],
                        Box::new(FilterPass {
                            output: filtered,
                            group: vgroup,
                            filter: Filter::Vertical,
                            viewport: [0, 0, v.0, v.1],
                            scissor: None,
                        }),
                    )
                    .read(ResourceRef::Attachment(hx))
                    .read(ResourceRef::Buffer(vgroup))
                    .write(ResourceRef::Attachment(filtered))
                    .depends_on(horizontal),
                );
                let replaced = attachment(&mut b, format, size);
                roles.insert(replaced, PhysicalRole::Accumulator);
                let rgroup = uniform(&mut b);
                groups.push((rgroup, Filter::Replace, n));
                previous = Some(
                    b.add_pass(
                        RenderPassNode::new(
                            REPLACE[n],
                            Box::new(FilterPass {
                                output: replaced,
                                group: rgroup,
                                filter: Filter::Replace,
                                viewport: p.visible,
                                 scissor: None,
                            }),
                        )
                        .read(ResourceRef::Attachment(snapshot))
                        .read(ResourceRef::Attachment(filtered))
                        .read(ResourceRef::Attachment(out))
                        .read(ResourceRef::Buffer(rgroup))
                        .write(ResourceRef::Attachment(replaced))
                        .depends_on(vertical),
                    ),
                );
                version = Some(replaced);
                }
                if blooms {
                    let selected = bloom.as_ref().expect("qualified Bloom marker");
                    let regions = selected.regions();
                    let mut input = None;
                    let old = version.unwrap();
                    for stage in BloomStage::ALL {
                        let i = stage as usize;
                        let composite = stage == BloomStage::Composite;
                        let (output, viewport) = if composite {
                            let output = attachment(&mut b, format, size);
                            roles.insert(output, PhysicalRole::Accumulator);
                            (output, selected.composite.output_roi)
                        } else {
                            let [_, _, w, h] = regions[i];
                            let output = attachment(&mut b, HDR_FORMAT, (w, h));
                            roles.insert(output, PhysicalRole::Bloom(i));
                            (output, [0, 0, w, h])
                        };
                        let group = b.declare_buffer(BufferDesc {
                            label: stage.name(),
                            size: if stage == BloomStage::Source { selected.source_stride } else { stage.uniform_size() },
                            usage: wgpu::BufferUsages::UNIFORM,
                        });
                        let filter = Filter::Bloom(stage);
                        groups.push((group, filter, 0));
                        let mut pass = RenderPassNode::new(stage.name(), Box::new(FilterPass {
                            output, group, filter, viewport,
                            scissor: composite.then_some(selected.scissor),
                        }))
                        .read(ResourceRef::Buffer(group))
                        .write(ResourceRef::Attachment(output))
                        .depends_on(previous.unwrap());
                        if let Some(input) = input {
                            pass = pass.read(ResourceRef::Attachment(input));
                        }
                        if composite {
                            // The previous physical A is loaded as an attachment.
                            // This dependency does not bind it as a shader input.
                            pass = pass.read(ResourceRef::Attachment(old));
                            version = Some(output);
                        }
                        previous = Some(b.add_pass(pass));
                        input = Some(output);
                    }
                }
                if at == commands.len() { break; }
                start = at + 1;
            }
        }
        let output = attachment(&mut b, format, size);
        roles.insert(output, PhysicalRole::Terminal);
        let group = uniform(&mut b);
        groups.push((group, Filter::Terminal, 0));
        b.add_pass(
            RenderPassNode::new(
                "frost_terminal",
                Box::new(FilterPass {
                    output,
                    group,
                    filter: Filter::Terminal,
                    viewport: [0, 0, size.0, size.1],
                    scissor: None,
                }),
            )
            .read(ResourceRef::Attachment(version.unwrap()))
            .read(ResourceRef::Buffer(group))
            .write(ResourceRef::Attachment(output))
            .depends_on(previous.unwrap()),
        );
        let topology = b.build(draw_plan).map_err(Refusal::Graph)?;
        // Every pass is in the unique dependency chain. Aliasing A never changes
        // graph writer identity or introduces a future-writer dependency cycle.
        if topology.passes_in_order().count() != segment + (occurrences.len() + usize::from(bloom.is_some())) * 4 + 1 {
            return Err(Refusal::Graph(GraphError::DuplicatePassName {
                name: "frost chain",
            }));
        }
        validate_alias_order(&topology, &roles).map_err(Refusal::Graph)?;
        Ok(Self {
            topology,
            output,
            occurrences,
            bloom,
            roles,
            groups,
            size,
            format,
            bytes,
        })
    }
}

/// HAL385 §8.5: validate physical A aliasing against the completed logical
/// schedule, rather than relying on the graph's final-writer map for safety.
fn validate_alias_order(
    topology: &RenderGraphTopology,
    roles: &HashMap<AttachmentHandle, PhysicalRole>,
) -> Result<(), GraphError> {
    let passes: Vec<_> = topology.passes_in_order().collect();
    let mut versions = Vec::new();
    for (&handle, role) in roles {
        if *role != PhysicalRole::Accumulator {
            continue;
        }
        let resource = ResourceRef::Attachment(handle);
        let writers: Vec<_> = passes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.writes.contains(&resource))
            .map(|(i, _)| i)
            .collect();
        if writers.len() != 1 {
            return Err(GraphError::CycleDetected {
                path: vec!["HAL385 single-writer version"],
            });
        }
        let last_reader = passes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.reads.contains(&resource))
            .map(|(i, _)| i)
            .max()
            .unwrap_or(writers[0]);
        versions.push((writers[0], last_reader));
    }
    versions.sort_unstable();
    if versions.windows(2).any(|v| v[0].1 > v[1].0) {
        return Err(GraphError::CycleDetected {
            path: vec!["HAL385 reader before accumulator overwrite"],
        });
    }
    Ok(())
}
struct NormalSegment {
    output: AttachmentHandle,
    range: Range<usize>,
    scope: PassDrawScope,
    clear: bool,
}
impl PassExecutor for NormalSegment {
    fn execute(&self, c: &mut PassContext<'_>) -> Result<(), PassExecutionError> {
        let view = c.resources.get_attachment(self.output).ok_or(
            PassExecutionError::missing_attachment(c.pass_name, self.output),
        )?;
        let color = c.constants.clear_color;
        let mut p = c.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(c.pass_name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if self.clear {
                        wgpu::LoadOp::Clear(wgpu::Color {
                            r: color.r as f64,
                            g: color.g as f64,
                            b: color.b as f64,
                            a: color.a as f64,
                        })
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        super::super::render_graph::passes::pipeline_dispatch::encode_draw_commands_range_for_scope(
            &mut p,
            c.constants,
            c.draw_data,
            c.render_resources,
            c.buffers,
            self.scope,
            self.range.clone(),
        );
        Ok(())
    }
}
struct CopySnapshot {
    source: AttachmentHandle,
    destination: AttachmentHandle,
    roi: [u32; 4],
}
impl PassExecutor for CopySnapshot {
    fn execute(&self, c: &mut PassContext<'_>) -> Result<(), PassExecutionError> {
        let resources =
            &c.resources
                .frosted
                .as_ref()
                .ok_or(PassExecutionError::missing_attachment(
                    c.pass_name,
                    self.source,
                ))?;
        let source =
            resources
                .textures
                .get(&self.source)
                .ok_or(PassExecutionError::missing_attachment(
                    c.pass_name,
                    self.source,
                ))?;
        let destination = resources.textures.get(&self.destination).ok_or(
            PassExecutionError::missing_attachment(c.pass_name, self.destination),
        )?;
        c.encoder.copy_texture_to_texture(
            wgpu::TexelCopyTextureInfo {
                texture: source,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: self.roi[0],
                    y: self.roi[1],
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyTextureInfo {
                texture: destination,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::Extent3d {
                width: self.roi[2],
                height: self.roi[3],
                depth_or_array_layers: 1,
            },
        );
        Ok(())
    }
}
struct FilterPass {
    output: AttachmentHandle,
    group: BufferHandle,
    filter: Filter,
    viewport: [u32; 4],
    scissor: Option<[u32; 4]>,
}
impl PassExecutor for FilterPass {
    fn execute(&self, c: &mut PassContext<'_>) -> Result<(), PassExecutionError> {
        let view = c.resources.get_attachment(self.output).ok_or(
            PassExecutionError::missing_attachment(c.pass_name, self.output),
        )?;
        let r = c
            .resources
            .frosted
            .as_ref()
            .ok_or(PassExecutionError::missing_buffer(c.pass_name, self.group))?;
        let group = r
            .groups
            .get(&self.group)
            .ok_or(PassExecutionError::missing_buffer(c.pass_name, self.group))?;
        let pipeline = match self.filter {
            Filter::Horizontal => &r.pipelines.horizontal,
            Filter::Vertical => &r.pipelines.vertical,
            Filter::Replace => &r.pipelines.replace,
            Filter::Terminal => &r.pipelines.terminal,
            Filter::Bloom(stage) => r.bloom.as_ref()
                .ok_or(PassExecutionError::missing_buffer(c.pass_name, self.group))?
                .get(stage),
        };
        let mut p = c.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some(c.pass_name),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                depth_slice: None,
                view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if matches!(self.filter, Filter::Replace | Filter::Bloom(BloomStage::Composite)) {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            multiview_mask: None,
            occlusion_query_set: None,
        });
        p.set_pipeline(pipeline);
        p.set_bind_group(
            0,
            c.render_resources.rect_globals_bind_group,
            &[c.draw_data.target.globals_offset],
        );
        let offsets: &[u32] = if matches!(self.filter, Filter::Bloom(BloomStage::Source)) { &[0] } else { &[] };
        p.set_bind_group(1, group, offsets);
        let v = self.viewport;
        p.set_viewport(v[0] as f32, v[1] as f32, v[2] as f32, v[3] as f32, 0.0, 1.0);
        let [x, y, w, h] = self.scissor.unwrap_or(v);
        p.set_scissor_rect(x, y, w, h);
        p.draw(0..3, 0..1);
        Ok(())
    }
}
