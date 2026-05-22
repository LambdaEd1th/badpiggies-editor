// Port of Unity shader "_Custom/PreAlpha_Unlit_ColorTransparent_Geometry_Gray".

struct Uniforms {
    object_to_clip: mat4x4<f32>,
    main_tex_st: vec4<f32>,
    color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) position: vec4<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

fn transform_tex(uv: vec2<f32>, st: vec4<f32>) -> vec2<f32> {
    return uv * st.xy + st.zw;
}

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.position = u.object_to_clip * in.position;
    out.uv = transform_tex(in.uv, u.main_tex_st);
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let tex = textureSample(main_tex, main_sampler, in.uv);
    let lum = 0.2989 * tex.x + 0.587 * tex.y + 0.114 * tex.z;
    return vec4<f32>(lum, lum, lum, tex.w) * u.color;
}