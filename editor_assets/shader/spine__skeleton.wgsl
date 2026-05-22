// Port of Unity shader "Spine/Skeleton".
// Blend SrcAlpha OneMinusSrcAlpha, ZWrite Off, Cull Off

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) color: vec4<f32>,
    @location(1) position: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.color = clamp(in.color, vec4<f32>(0.0), vec4<f32>(1.0));
    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(main_tex, main_sampler, in.uv) * in.color;
}// Port of Unity shader "Spine/Skeleton".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VIn {
    @location(0) color: vec4<f32>,
    @location(1) position: vec4<f32>,
    @location(2) uv: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    out.color = clamp(in.color, vec4<f32>(0.0), vec4<f32>(1.0));
    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(main_tex, main_sampler, in.uv) * in.color;
}