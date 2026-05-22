//! wgpu + WGSL dark-overlay shader set.
//!
//! This module ports the Unity dark-overlay material stack used by the editor:
//! - `Depth Mask/Unlit Transparent (CG)` for the border-ring multiply pass
//! - `Depth Mask/MaskOverlay` for the fullscreen dark complement pass
//! - `Depth Mask/MaskOverlayNV` for the night-vision fullscreen vignette pass

use std::sync::Arc;

use eframe::egui;
use eframe::wgpu;

const MAX_DRAW_SLOTS: u32 = 64;

pub const DEPTH_MASK_TRANSPARENT_COLOR: [f32; 4] =
    [0.686_274_5, 0.686_274_5, 0.686_274_5, 0.686_274_5];
pub const MASK_OVERLAY_COLOR: [f32; 4] = [0.045_877_58, 0.058_823_53, 0.045_415_22, 1.0];
pub const DEPTH_MASK_TRANSPARENT_NIGHT_VISION_COLOR: [f32; 4] =
    [0.794_117_6, 0.794_117_6, 0.794_117_6, 0.682_353];
pub const MASK_OVERLAY_NIGHT_VISION_COLOR: [f32; 4] =
    [0.045_877_58, 0.058_823_53, 0.045_415_22, 0.556_862_8];
pub const NIGHT_VISION_OVERLAY_COLOR: [f32; 4] = [0.05, 0.426_470_6, 0.0, 0.0];
pub const NIGHT_VISION_OVERLAY_RADIUS: f32 = 0.6;
pub const NIGHT_VISION_OVERLAY_SOFTNESS: f32 = 0.3;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum DarkMaskPipelineKind {
    Multiply,
    Alpha,
    NightVisionOverlay,
}

const UNLIT_TRANSPARENT_CG_RUNTIME_WGSL: &str =
    include_str!("../../assets/shader/depth_mask__unlit_transparent_cg__runtime.wgsl");
const MASK_OVERLAY_RUNTIME_WGSL: &str =
    include_str!("../../assets/shader/depth_mask__maskoverlay__runtime.wgsl");
const MASK_OVERLAY_NV_RUNTIME_WGSL: &str =
    include_str!("../../assets/shader/depth_mask__maskoverlaynv__runtime.wgsl");

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct DarkMaskUniforms {
    pub viewport_min: [f32; 2],
    pub viewport_size: [f32; 2],
    pub color: [f32; 4],
    pub params: [f32; 4],
}

impl DarkMaskUniforms {
    pub fn for_viewport(rect: egui::Rect, color: [f32; 4]) -> Self {
        Self::for_viewport_with_params(rect, color, [0.0; 4])
    }

    pub fn for_viewport_with_params(rect: egui::Rect, color: [f32; 4], params: [f32; 4]) -> Self {
        Self {
            viewport_min: [rect.min.x, rect.min.y],
            viewport_size: [rect.width(), rect.height()],
            color,
            params,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct DarkMaskVertex {
    pos: [f32; 2],
}

pub struct DarkMaskResources {
    multiply_pipeline: wgpu::RenderPipeline,
    alpha_pipeline: wgpu::RenderPipeline,
    night_vision_overlay_pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    uniform_buffer: wgpu::Buffer,
    slot_stride: u64,
}

pub fn init_dark_mask_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> DarkMaskResources {
    let unlit_transparent_cg_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("depth_mask__unlit_transparent_cg_runtime_shader"),
        source: wgpu::ShaderSource::Wgsl(UNLIT_TRANSPARENT_CG_RUNTIME_WGSL.into()),
    });
    let mask_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("depth_mask__maskoverlay_runtime_shader"),
        source: wgpu::ShaderSource::Wgsl(MASK_OVERLAY_RUNTIME_WGSL.into()),
    });
    let mask_overlay_nv_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("depth_mask__maskoverlaynv_runtime_shader"),
        source: wgpu::ShaderSource::Wgsl(MASK_OVERLAY_NV_RUNTIME_WGSL.into()),
    });

    let min_align = device.limits().min_uniform_buffer_offset_alignment as u64;
    let uniform_size = std::mem::size_of::<DarkMaskUniforms>() as u64;
    let slot_stride = uniform_size.div_ceil(min_align) * min_align;

    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("depth_mask__runtime_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: true,
                min_binding_size: std::num::NonZeroU64::new(uniform_size),
            },
            count: None,
        }],
    });

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("depth_mask__runtime_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });

    let vertex = wgpu::VertexState {
        module: &unlit_transparent_cg_shader,
        entry_point: Some("vs_main"),
        buffers: &[wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<DarkMaskVertex>() as u64,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 0,
                shader_location: 0,
            }],
        }],
        compilation_options: Default::default(),
    };

    let primitive = wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: None,
        ..Default::default()
    };

    let multiply_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("depth_mask__unlit_transparent_cg__multiply_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: vertex.clone(),
        fragment: Some(wgpu::FragmentState {
            module: &unlit_transparent_cg_shader,
            entry_point: Some("fs_color"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::Dst,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::DstAlpha,
                        dst_factor: wgpu::BlendFactor::Zero,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive,
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let alpha_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("depth_mask__maskoverlay__alpha_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: vertex.clone(),
        fragment: Some(wgpu::FragmentState {
            module: &mask_overlay_shader,
            entry_point: Some("fs_color"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState {
                    color: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                    alpha: wgpu::BlendComponent {
                        src_factor: wgpu::BlendFactor::SrcAlpha,
                        dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                        operation: wgpu::BlendOperation::Add,
                    },
                }),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive,
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });

    let night_vision_overlay_pipeline =
        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("depth_mask__maskoverlaynv__night_vision_overlay_pipeline"),
            layout: Some(&pipeline_layout),
            vertex,
            fragment: Some(wgpu::FragmentState {
                module: &mask_overlay_nv_shader,
                entry_point: Some("fs_night_vision_overlay"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: target_format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Src,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::Src,
                            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::COLOR,
                })],
                compilation_options: Default::default(),
            }),
            primitive,
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("depth_mask__runtime_uniform_buffer"),
        size: slot_stride * MAX_DRAW_SLOTS as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("depth_mask__runtime_bind_group"),
        layout: &bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &uniform_buffer,
                offset: 0,
                size: std::num::NonZeroU64::new(uniform_size),
            }),
        }],
    });

    DarkMaskResources {
        multiply_pipeline,
        alpha_pipeline,
        night_vision_overlay_pipeline,
        bind_group,
        uniform_buffer,
        slot_stride,
    }
}

pub struct DarkMaskGpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

pub fn build_dark_mask_gpu_mesh(
    device: &wgpu::Device,
    mesh: &egui::Mesh,
) -> Option<DarkMaskGpuMesh> {
    if mesh.vertices.is_empty() || mesh.indices.is_empty() {
        return None;
    }

    use wgpu::util::DeviceExt;

    let vertices: Vec<DarkMaskVertex> = mesh
        .vertices
        .iter()
        .map(|vertex| DarkMaskVertex {
            pos: [vertex.pos.x, vertex.pos.y],
        })
        .collect();

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("depth_mask__runtime_vertex_buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("depth_mask__runtime_index_buffer"),
        contents: bytemuck::cast_slice(&mesh.indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    Some(DarkMaskGpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: mesh.indices.len() as u32,
    })
}

struct DarkMaskPaintCallback {
    resources: Arc<DarkMaskResources>,
    gpu_mesh: Arc<DarkMaskGpuMesh>,
    pipeline_kind: DarkMaskPipelineKind,
    slot: u32,
    uniforms: DarkMaskUniforms,
}

impl egui_wgpu::CallbackTrait for DarkMaskPaintCallback {
    fn prepare(
        &self,
        _device: &wgpu::Device,
        queue: &wgpu::Queue,
        _screen_descriptor: &egui_wgpu::ScreenDescriptor,
        _encoder: &mut wgpu::CommandEncoder,
        _callback_resources: &mut egui_wgpu::CallbackResources,
    ) -> Vec<wgpu::CommandBuffer> {
        let offset = self.slot as u64 * self.resources.slot_stride;
        queue.write_buffer(
            &self.resources.uniform_buffer,
            offset,
            bytemuck::bytes_of(&self.uniforms),
        );
        Vec::new()
    }

    fn paint(
        &self,
        _info: egui::PaintCallbackInfo,
        render_pass: &mut wgpu::RenderPass<'static>,
        _callback_resources: &egui_wgpu::CallbackResources,
    ) {
        let offset = self.slot as u64 * self.resources.slot_stride;
        let pipeline = match self.pipeline_kind {
            DarkMaskPipelineKind::Multiply => &self.resources.multiply_pipeline,
            DarkMaskPipelineKind::Alpha => &self.resources.alpha_pipeline,
            DarkMaskPipelineKind::NightVisionOverlay => {
                &self.resources.night_vision_overlay_pipeline
            }
        };
        render_pass.set_pipeline(pipeline);
        render_pass.set_bind_group(0, &self.resources.bind_group, &[offset as u32]);
        render_pass.set_vertex_buffer(0, self.gpu_mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(
            self.gpu_mesh.index_buffer.slice(..),
            wgpu::IndexFormat::Uint32,
        );
        render_pass.draw_indexed(0..self.gpu_mesh.index_count, 0, 0..1);
    }
}

pub fn make_dark_mask_callback(
    clip_rect: egui::Rect,
    resources: Arc<DarkMaskResources>,
    gpu_mesh: Arc<DarkMaskGpuMesh>,
    pipeline_kind: DarkMaskPipelineKind,
    slot: u32,
    uniforms: DarkMaskUniforms,
) -> egui::Shape {
    let cb = DarkMaskPaintCallback {
        resources,
        gpu_mesh,
        pipeline_kind,
        slot,
        uniforms,
    };
    egui::Shape::Callback(egui_wgpu::Callback::new_paint_callback(clip_rect, cb))
}

pub const fn max_draw_slots() -> u32 {
    MAX_DRAW_SLOTS
}
