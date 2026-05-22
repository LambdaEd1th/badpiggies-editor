// Port of Unity shader "_Custom/Utility_ZWrite".
// ColorMask 0, Cull Off, Fog Off.
// The shader body forwards vertex color; the caller must disable color writes in the pipeline.

struct Uniforms {
    object_to_clip: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexInput {
    @location(0) color: vec4<f32>,
    @location(1) position: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = clamp(in.color, vec4<f32>(0.0), vec4<f32>(1.0));
    out.position = u.object_to_clip * in.position;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}