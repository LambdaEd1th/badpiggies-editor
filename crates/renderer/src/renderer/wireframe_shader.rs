//! Cached terrain triangulation wireframe rendered as world-space line lists.

use std::rc::Rc;

use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct WireframeUniforms {
    pub screen_size: [f32; 2],
    pub camera_center: [f32; 2],
    pub zoom: f32,
    pub _pad0: f32,
    pub _pad1: [f32; 2],
    pub color: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct WireframeVertex {
    position: [f32; 2],
}

pub struct WireframeResources {
    pipeline: wgpu::RenderPipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

pub struct WireframeGpuMesh {
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
    uniform_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    bounds: [f32; 4],
}

fn triangle_line_indices(triangle_indices: &[u32], vertex_count: usize) -> Vec<u32> {
    let mut line_indices = Vec::with_capacity(triangle_indices.len() * 2);
    for triangle in triangle_indices.chunks_exact(3) {
        if triangle.iter().any(|&index| index as usize >= vertex_count) {
            continue;
        }
        line_indices.extend_from_slice(&[
            triangle[0],
            triangle[1],
            triangle[1],
            triangle[2],
            triangle[2],
            triangle[0],
        ]);
    }
    line_indices
}

pub fn init_wireframe_resources(
    device: &wgpu::Device,
    target_format: wgpu::TextureFormat,
) -> WireframeResources {
    let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("terrain_wireframe_shader"),
        source: wgpu::ShaderSource::Wgsl(
            r#"
struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    _pad0: f32,
    _pad1: vec2<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) position: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    let ndc = (input.position - u.camera_center) * u.zoom / (u.screen_size * 0.5);
    output.position = vec4<f32>(ndc, 0.0, 1.0);
    return output;
}

@fragment
fn fs_main() -> @location(0) vec4<f32> {
    return u.color;
}
"#
            .into(),
        ),
    });
    let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("terrain_wireframe_bind_group_layout"),
        entries: &[wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: std::num::NonZeroU64::new(
                    std::mem::size_of::<WireframeUniforms>() as u64,
                ),
            },
            count: None,
        }],
    });
    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("terrain_wireframe_pipeline_layout"),
        bind_group_layouts: &[Some(&bind_group_layout)],
        immediate_size: 0,
    });
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("terrain_wireframe_pipeline"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &shader,
            entry_point: Some("vs_main"),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: std::mem::size_of::<WireframeVertex>() as u64,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &[wgpu::VertexAttribute {
                    format: wgpu::VertexFormat::Float32x2,
                    offset: 0,
                    shader_location: 0,
                }],
            })],
            compilation_options: Default::default(),
        },
        fragment: Some(wgpu::FragmentState {
            module: &shader,
            entry_point: Some("fs_main"),
            targets: &[Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })],
            compilation_options: Default::default(),
        }),
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::LineList,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        multiview_mask: None,
        cache: None,
    });
    WireframeResources {
        pipeline,
        bind_group_layout,
    }
}

pub fn build_wireframe_gpu_mesh(
    device: &wgpu::Device,
    resources: &WireframeResources,
    mesh: &crate::gpu2d::Mesh,
) -> Option<WireframeGpuMesh> {
    if mesh.vertices.is_empty() || mesh.indices.len() < 3 {
        return None;
    }
    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|vertex| WireframeVertex {
            position: [vertex.pos.x, vertex.pos.y],
        })
        .collect();
    let line_indices = triangle_line_indices(&mesh.indices, vertices.len());
    if line_indices.is_empty() {
        return None;
    }

    let first = vertices[0].position;
    let bounds = vertices.iter().skip(1).fold(
        [first[0], first[1], first[0], first[1]],
        |mut bounds, vertex| {
            bounds[0] = bounds[0].min(vertex.position[0]);
            bounds[1] = bounds[1].min(vertex.position[1]);
            bounds[2] = bounds[2].max(vertex.position[0]);
            bounds[3] = bounds[3].max(vertex.position[1]);
            bounds
        },
    );
    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain_wireframe_vertices"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("terrain_wireframe_indices"),
        contents: bytemuck::cast_slice(&line_indices),
        usage: wgpu::BufferUsages::INDEX,
    });
    let uniform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrain_wireframe_uniforms"),
        size: std::mem::size_of::<WireframeUniforms>() as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("terrain_wireframe_bind_group"),
        layout: &resources.bind_group_layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    Some(WireframeGpuMesh {
        vertex_buffer,
        index_buffer,
        index_count: line_indices.len() as u32,
        uniform_buffer,
        bind_group,
        bounds,
    })
}

impl WireframeGpuMesh {
    pub fn is_visible(&self, screen_size: [f32; 2], camera_center: [f32; 2], zoom: f32) -> bool {
        let half_width = screen_size[0] * 0.5 / zoom.max(0.0001);
        let half_height = screen_size[1] * 0.5 / zoom.max(0.0001);
        self.bounds[2] >= camera_center[0] - half_width
            && self.bounds[0] <= camera_center[0] + half_width
            && self.bounds[3] >= camera_center[1] - half_height
            && self.bounds[1] <= camera_center[1] + half_height
    }
}

struct WireframePaintCallback {
    resources: Rc<WireframeResources>,
    mesh: Rc<WireframeGpuMesh>,
    uniforms: WireframeUniforms,
}

impl crate::gpu2d::PaintCallback for WireframePaintCallback {
    fn prepare(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.mesh.uniform_buffer,
            0,
            bytemuck::bytes_of(&self.uniforms),
        );
    }

    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>) {
        render_pass.set_pipeline(&self.resources.pipeline);
        render_pass.set_bind_group(0, &self.mesh.bind_group, &[]);
        render_pass.set_vertex_buffer(0, self.mesh.vertex_buffer.slice(..));
        render_pass.set_index_buffer(self.mesh.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        render_pass.draw_indexed(0..self.mesh.index_count, 0, 0..1);
    }
}

pub fn make_wireframe_callback(
    clip_rect: crate::gpu2d::Rect,
    resources: Rc<WireframeResources>,
    mesh: Rc<WireframeGpuMesh>,
    uniforms: WireframeUniforms,
) -> crate::gpu2d::Shape {
    crate::gpu2d::Shape::Callback(crate::gpu2d::Callback::new(
        clip_rect,
        WireframePaintCallback {
            resources,
            mesh,
            uniforms,
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::triangle_line_indices;

    #[test]
    fn expands_triangles_into_line_list_edges() {
        assert_eq!(
            triangle_line_indices(&[0, 1, 2, 2, 1, 3], 4),
            [0, 1, 1, 2, 2, 0, 2, 1, 1, 3, 3, 2]
        );
    }

    #[test]
    fn skips_invalid_triangles_and_incomplete_tail() {
        assert_eq!(
            triangle_line_indices(&[0, 1, 9, 0, 2], 3),
            Vec::<u32>::new()
        );
    }
}
