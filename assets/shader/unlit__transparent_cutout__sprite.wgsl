// Unity Unlit/Transparent Cutout.
// Fragment: clip(tex.a - _Cutoff); return tex * _Color;
// Blend Off, ZWrite On, Cull Back (editor uses Cull Off in 2D).

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    y_offset: f32,
    tint_color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var main_tex: texture_2d<f32>;
@group(0) @binding(2) var main_sampler: sampler;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    let sx = (in.position.x - u.camera_center.x) * u.zoom;
    let sy = (in.position.y + u.y_offset - u.camera_center.y) * u.zoom;
    out.position = vec4<f32>(
        sx / (u.screen_size.x * 0.5),
        sy / (u.screen_size.y * 0.5),
        0.0,
        1.0,
    );
    out.uv = in.uv;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let c = textureSample(main_tex, main_sampler, in.uv) * u.tint_color;
    if (c.a < 0.5) { discard; }
    return vec4<f32>(c.rgb, 1.0);
}