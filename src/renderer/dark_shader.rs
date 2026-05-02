//! GPU dark overlay with stencil buffer.
//!
//! Replicates Unity's depth-mask approach for dark level overlays:
//! - Offscreen render pass with stencil marks lit-area polygons
//! - Fill dark color where stencil==0 (outside), border color where stencil==1 (ring)
//! - Composite result onto the egui framebuffer via premultiplied alpha blend

use std::sync::{Arc, Mutex};

use eframe::{egui, wgpu};

const NUM_SLOTS: u64 = 3;

// ── WGSL: mark + fill passes (offscreen with stencil) ──

const WGSL_OFFSCREEN: &str = r#"
struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    _pad1: vec2<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

// Mark pass: world-space polygon → NDC, stencil write only.
@vertex fn vs_mark(@location(0) pos: vec2<f32>) -> @builtin(position) vec4<f32> {
    let ndc = (pos - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    return vec4<f32>(ndc, 0.0, 1.0);
}
@fragment fn fs_mark() -> @location(0) vec4<f32> {
    return vec4<f32>(0.0);
}

// Fill pass: fullscreen triangle with flat uniform color, stencil test.
@vertex fn vs_fill(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(i & 1u)) * 4.0 - 1.0;
    let y = f32(i32(i >> 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}
@fragment fn fs_fill() -> @location(0) vec4<f32> {
    return u.color;
}
"#;

// ── WGSL: composite pass (blit offscreen texture → egui framebuffer) ──

const WGSL_COMPOSITE: &str = r#"
@group(0) @binding(0) var dark_tex: texture_2d<f32>;
@group(0) @binding(1) var dark_samp: sampler;

@vertex fn vs_comp(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    let x = f32(i32(i & 1u)) * 4.0 - 1.0;
    let y = f32(i32(i >> 1u)) * 4.0 - 1.0;
    return vec4<f32>(x, y, 0.0, 1.0);
}

@fragment fn fs_comp(@builtin(position) pos: vec4<f32>) -> @location(0) vec4<f32> {
    let dims = vec2<f32>(textureDimensions(dark_tex));
    let uv = pos.xy / dims;
    return textureSample(dark_tex, dark_samp, uv);
}
"#;

// ── Uniform layout (48 bytes, 16-aligned) ──

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DarkUniforms {
    screen_size: [f32; 2],
    camera_center: [f32; 2],
    zoom: f32,
    _pad0: f32,
    _pad1: [f32; 2],
    color: [f32; 4], // premultiplied RGBA
}

// ── Shared GPU resources (created once at startup) ──

pub struct DarkResources {
    /// Mark border polygons: stencil Invert on bit 0 (write_mask=0x01).
    mark_border_pipeline: wgpu::RenderPipeline,
    /// Mark inner polygons: stencil Invert on bit 1 (write_mask=0x02).
    mark_inner_pipeline: wgpu::RenderPipeline,
    fill_pipeline: wgpu::RenderPipeline,
    composite_pipeline: wgpu::RenderPipeline,
    uniform_bgl: wgpu::BindGroupLayout,
    composite_bgl: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    uniform_buffer: wgpu::Buffer,
    slot_stride: u64,
    offscreen_format: wgpu::TextureFormat,
    offscreen: Mutex<Option<DarkOffscreen>>,
}

// ── Offscreen render targets (resized lazily) ──

struct DarkOffscreen {
    color_view: wgpu::TextureView,
    stencil_view: wgpu::TextureView,
    composite_bind_group: wgpu::BindGroup,
    width: u32,
    height: u32,
}

// ── Pre-built fan-triangulated GPU meshes (built on level load) ──

pub struct DarkGpuMeshes {
    border_vbo: wgpu::Buffer,
    border_ibo: wgpu::Buffer,
    border_count: u32,
    inner_vbo: wgpu::Buffer,
    inner_ibo: wgpu::Buffer,
    inner_count: u32,
    has_data: bool,
}

// ── Initialization ──

pub fn init_dark_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> DarkResources {
    let offscreen_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("dark_offscreen"),
        source: wgpu::ShaderSource::Wgsl(WGSL_OFFSCREEN.into()),
    });
    let composite_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("dark_composite"),
        source: wgpu::ShaderSource::Wgsl(WGSL_COMPOSITE.into()),
    });

    let min_align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let u_size = std::mem::size_of::<DarkUniforms>() as u64;
    let slot_stride = u_size.div_ceil(min_align) * min_align;

    // Uniform bind group layout (shared by mark + fill pipelines)
    let uniform_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("dark_uniform_bgl"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: std::num::NonZeroU64::new(u_size),
            },
            count: None,
        }],
    });

    let offscreen_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("dark_offscreen_pl"),
        bind_group_layouts: &[Some(&uniform_bgl)],
        immediate_size: 0,
    });

    let offscreen_format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let stencil_format = wgpu::TextureFormat::Depth24PlusStencil8;

    // Stencil Invert for even-odd fill rule (handles concave polygons correctly).
    // Border mark writes bit 0, inner mark writes bit 1.
    // After both passes: 0b00=outside, 0b01=border ring, 0b11=lit interior.
    let stencil_invert = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Always,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Invert,
    };
    let stencil_test = wgpu::StencilFaceState {
        compare: wgpu::CompareFunction::Equal,
        fail_op: wgpu::StencilOperation::Keep,
        depth_fail_op: wgpu::StencilOperation::Keep,
        pass_op: wgpu::StencilOperation::Keep,
    };

    let mark_vertex_state = wgpu::VertexState {
        module: &offscreen_shader,
        entry_point: Some("vs_mark"),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: 8, // [f32; 2]
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        }],
        compilation_options: Default::default(),
    };
    let mark_fragment_state = wgpu::FragmentState {
        module: &offscreen_shader,
        entry_point: Some("fs_mark"),
        targets: &[Some(wgpu::ColorTargetState {
            format: offscreen_format,
            blend: None,
            write_mask: wgpu::ColorWrites::empty(),
        })],
        compilation_options: Default::default(),
    };
    let mark_primitive = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: None,
        ..Default::default()
    };

    // Mark border pipeline: Invert on bit 0
    let mark_border_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dark_mark_border"),
        layout: Some(&offscreen_pl),
        vertex: mark_vertex_state.clone(),
        fragment: Some(mark_fragment_state.clone()),
        primitive: mark_primitive,
        depth_stencil: Some(wgpu::DepthStencilState {
            format: stencil_format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: stencil_invert,
                back: stencil_invert,
                read_mask: 0xFF,
                write_mask: 0x01,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // Mark inner pipeline: Invert on bit 1
    let mark_inner_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dark_mark_inner"),
        layout: Some(&offscreen_pl),
        vertex: mark_vertex_state,
        fragment: Some(mark_fragment_state),
        primitive: mark_primitive,
        depth_stencil: Some(wgpu::DepthStencilState {
            format: stencil_format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: stencil_invert,
                back: stencil_invert,
                read_mask: 0xFF,
                write_mask: 0x02,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // Fill pipeline: stencil test, flat color output
    let fill_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dark_fill"),
        layout: Some(&offscreen_pl),
        vertex: wgpu::VertexState {
            module: &offscreen_shader,
            entry_point: Some("vs_fill"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &offscreen_shader,
            entry_point: Some("fs_fill"),
            targets: &[Some(wgpu::ColorTargetState {
                format: offscreen_format,
                blend: None,
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: stencil_format,
            depth_write_enabled: Some(false),
            depth_compare: Some(wgpu::CompareFunction::Always),
            stencil: wgpu::StencilState {
                front: stencil_test,
                back: stencil_test,
                read_mask: 0xFF,
                write_mask: 0x00,
            },
            bias: wgpu::DepthBiasState::default(),
        }),
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    // Composite bind group layout (texture + sampler)
    let composite_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("dark_composite_bgl"),
        entries: &[
            wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            wgpu::BindGroupLayoutEntry {
                binding: 1,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    });

    let composite_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("dark_composite_pl"),
        bind_group_layouts: &[Some(&composite_bgl)],
        immediate_size: 0,
    });

    // Composite pipeline: blit offscreen → egui framebuffer
    let composite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("dark_composite"),
        layout: Some(&composite_pl),
        vertex: wgpu::VertexState {
            module: &composite_shader,
            entry_point: Some("vs_comp"),
            buffers: &[],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &composite_shader,
            entry_point: Some("fs_comp"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("dark_sampler"),
        mag_filter: wgpu::FilterMode::Linear,
        min_filter: wgpu::FilterMode::Linear,
        ..Default::default()
    });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("dark_uniforms"),
        size: slot_stride * NUM_SLOTS,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    DarkResources {
        mark_border_pipeline,
        mark_inner_pipeline,
        fill_pipeline,
        composite_pipeline,
        uniform_bgl,
        composite_bgl,
        sampler,
        uniform_buffer,
        slot_stride,
        offscreen_format,
        offscreen: Mutex::new(None),
    }
}

// ── Offscreen texture management ──

impl DarkOffscreen {
    fn new(device: &wgpu::Device, res: &DarkResources, width: u32, height: u32) -> Self {
        let color_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dark_color_rt"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: res.offscreen_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color_tex.create_view(&Default::default());

        let stencil_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("dark_stencil_rt"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24PlusStencil8,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let stencil_view = stencil_tex.create_view(&Default::default());

        let composite_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dark_composite_bg"),
            layout: &res.composite_bgl,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&color_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&res.sampler),
                },
            ],
        });

        Self {
            color_view,
            stencil_view,
            composite_bind_group,
            width,
            height,
        }
    }
}

// ── Fan triangulation: polygon → GPU vertex/index data ──

#[allow(dead_code)]
pub fn build_dark_gpu_meshes<'a>(
    device: &wgpu::Device,
    pairs: impl Iterator<Item = (&'a [(f32, f32)], &'a [(f32, f32)])>,
) -> DarkGpuMeshes {
    use wgpu::util::DeviceExt;

    let mut border_verts: Vec<[f32; 2]> = Vec::new();
    let mut border_idxs: Vec<u32> = Vec::new();
    let mut inner_verts: Vec<[f32; 2]> = Vec::new();
    let mut inner_idxs: Vec<u32> = Vec::new();

    for (border, inner) in pairs {
        fan_triangulate(border, &mut border_verts, &mut border_idxs);
        fan_triangulate(inner, &mut inner_verts, &mut inner_idxs);
    }

    let has_data = !border_verts.is_empty();

    let border_vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dark_border_vbo"),
        contents: if border_verts.is_empty() {
            &[0u8; 8]
        } else {
            bytemuck::cast_slice(&border_verts)
        },
        usage: wgpu::BufferUsages::VERTEX,
    });
    let border_ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dark_border_ibo"),
        contents: if border_idxs.is_empty() {
            &[0u8; 4]
        } else {
            bytemuck::cast_slice(&border_idxs)
        },
        usage: wgpu::BufferUsages::INDEX,
    });
    let inner_vbo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dark_inner_vbo"),
        contents: if inner_verts.is_empty() {
            &[0u8; 8]
        } else {
            bytemuck::cast_slice(&inner_verts)
        },
        usage: wgpu::BufferUsages::VERTEX,
    });
    let inner_ibo = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("dark_inner_ibo"),
        contents: if inner_idxs.is_empty() {
            &[0u8; 4]
        } else {
            bytemuck::cast_slice(&inner_idxs)
        },
        usage: wgpu::BufferUsages::INDEX,
    });

    DarkGpuMeshes {
        border_vbo,
        border_ibo,
        border_count: border_idxs.len() as u32,
        inner_vbo,
        inner_ibo,
        inner_count: inner_idxs.len() as u32,
        has_data,
    }
}

/// Fan-triangulate a closed polygon from vertex[0].
///
/// Combined with stencil Invert, this implements correct even-odd fill
/// for arbitrary (convex or concave) polygons.
#[allow(dead_code)]
fn fan_triangulate(polygon: &[(f32, f32)], verts: &mut Vec<[f32; 2]>, idxs: &mut Vec<u32>) {
    let n = polygon.len();
    if n < 3 {
        return;
    }
    let base = verts.len() as u32;
    for &(x, y) in polygon {
        verts.push([x, y]);
    }
    // Fan from vertex 0: triangles (0, i, i+1) for i in 1..n-1
    for i in 1..n as u32 - 1 {
        idxs.push(base);
        idxs.push(base + i);
        idxs.push(base + i + 1);
    }
}

// ── Paint callback ──

struct DarkPaintCallback {
    resources: Arc<DarkResources>,
    gpu_meshes: Arc<DarkGpuMeshes>,
    camera_center: [f32; 2],
    /// [left, top, width, height] in egui logical points.
    canvas_rect: [f32; 4],
    zoom: f32,
}

impl egui_wgpu::CallbackTrait for DarkPaintCallback {
    fn prepare(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_descriptor: &egui_wgpu::ScreenDescriptor,
        encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        if !self.gpu_meshes.has_data {
            return Vec::new();
        }

        let ppp = screen_descriptor.pixels_per_point;
        let [scr_w, scr_h] = screen_descriptor.size_in_pixels;

        // Canvas viewport in pixels
        let vp_x = self.canvas_rect[0] * ppp;
        let vp_y = self.canvas_rect[1] * ppp;
        let vp_w = (self.canvas_rect[2] * ppp).max(1.0);
        let vp_h = (self.canvas_rect[3] * ppp).max(1.0);

        // ── Write uniform slots ──
        // Slot 0: camera (used by mark vertex shader)
        let camera_u = DarkUniforms {
            screen_size: [self.canvas_rect[2], self.canvas_rect[3]],
            camera_center: self.camera_center,
            zoom: self.zoom,
            _pad0: 0.0,
            _pad1: [0.0; 2],
            color: [0.0; 4],
        };
        queue.write_buffer(
            &self.resources.uniform_buffer,
            0,
            bytemuck::bytes_of(&camera_u),
        );

        // Slot 1: dark fill color (premultiplied)
        let dark_a: f32 = 200.0 / 255.0;
        let dark_u = DarkUniforms {
            color: [0.0, 0.0, 0.0, dark_a],
            ..bytemuck::Zeroable::zeroed()
        };
        queue.write_buffer(
            &self.resources.uniform_buffer,
            self.resources.slot_stride,
            bytemuck::bytes_of(&dark_u),
        );

        // Slot 2: border ring color (premultiplied)
        let border_a: f32 = 80.0 / 255.0;
        let border_u = DarkUniforms {
            color: [0.0, 0.0, 0.0, border_a],
            ..bytemuck::Zeroable::zeroed()
        };
        queue.write_buffer(
            &self.resources.uniform_buffer,
            self.resources.slot_stride * 2,
            bytemuck::bytes_of(&border_u),
        );

        // ── Ensure offscreen textures match screen size ──
        let mut ofs_guard = self.resources.offscreen.lock().unwrap();
        if ofs_guard.as_ref().map(|o| (o.width, o.height)) != Some((scr_w, scr_h)) {
            *ofs_guard = Some(DarkOffscreen::new(device, &self.resources, scr_w, scr_h));
        }
        let ofs = ofs_guard.as_ref().unwrap();

        // ── Uniform bind group ──
        let uniform_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("dark_uniform_bg"),
            layout: &self.resources.uniform_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.resources.uniform_buffer,
                    offset: 0,
                    size: std::num::NonZeroU64::new(std::mem::size_of::<DarkUniforms>() as u64),
                }),
            }],
        });

        // ── Offscreen stencil render pass ──
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("dark_offscreen"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &ofs.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &ofs.stencil_view,
                    depth_ops: None,
                    stencil_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0),
                        store: wgpu::StoreOp::Discard,
                    }),
                }),
                ..Default::default()
            });

            pass.set_viewport(vp_x, vp_y, vp_w, vp_h, 0.0, 1.0);

            // 1. Mark border polygons → stencil bit 0 (Invert, even-odd rule)
            pass.set_pipeline(&self.resources.mark_border_pipeline);
            pass.set_bind_group(0, &uniform_bg, &[0]);
            pass.set_vertex_buffer(0, self.gpu_meshes.border_vbo.slice(..));
            pass.set_index_buffer(
                self.gpu_meshes.border_ibo.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..self.gpu_meshes.border_count, 0, 0..1);

            // 2. Mark inner polygons → stencil bit 1 (Invert, even-odd rule)
            pass.set_pipeline(&self.resources.mark_inner_pipeline);
            pass.set_vertex_buffer(0, self.gpu_meshes.inner_vbo.slice(..));
            pass.set_index_buffer(
                self.gpu_meshes.inner_ibo.slice(..),
                wgpu::IndexFormat::Uint32,
            );
            pass.draw_indexed(0..self.gpu_meshes.inner_count, 0, 0..1);

            // 3. Fill dark where stencil == 0 (outside all light areas)
            pass.set_pipeline(&self.resources.fill_pipeline);
            pass.set_bind_group(0, &uniform_bg, &[self.resources.slot_stride as u32]);
            pass.set_stencil_reference(0);
            pass.draw(0..3, 0..1);

            // 4. Fill border where stencil == 1 (border ring zone)
            pass.set_bind_group(0, &uniform_bg, &[(self.resources.slot_stride * 2) as u32]);
            pass.set_stencil_reference(1);
            pass.draw(0..3, 0..1);
        }

        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        if !self.gpu_meshes.has_data {
            return;
        }
        let ofs_guard = self.resources.offscreen.lock().unwrap();
        if let Some(ref ofs) = *ofs_guard {
            render_pass.set_pipeline(&self.resources.composite_pipeline);
            render_pass.set_bind_group(0, &ofs.composite_bind_group, &[]);
            render_pass.draw(0..3, 0..1);
        }
    }
}

/// Build a paint callback shape for the GPU dark overlay.
pub fn make_dark_callback(
    clip_rect: egui::Rect,
    resources: Arc<DarkResources>,
    gpu_meshes: Arc<DarkGpuMeshes>,
    camera_center: [f32; 2],
    canvas_rect: [f32; 4],
    zoom: f32,
) -> egui::Shape {
    let cb = DarkPaintCallback {
        resources,
        gpu_meshes,
        camera_center,
        canvas_rect,
        zoom,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}
