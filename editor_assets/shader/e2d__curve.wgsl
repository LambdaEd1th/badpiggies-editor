// Port of the Unity terrain edge shader path used by the editor.
// Two splat textures blended via a per-node control texture.

struct Uniforms {
    screen_size: vec2<f32>,
    camera_center: vec2<f32>,
    zoom: f32,
    inv_control_size: f32,
    inv_control_size_half: f32,
    splat_params_x: f32,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var control_tex: texture_2d<f32>;
@group(0) @binding(2) var clamp_sampler: sampler;
@group(0) @binding(3) var splat0_tex: texture_2d<f32>;
@group(0) @binding(4) var repeat_sampler: sampler;
@group(0) @binding(5) var splat1_tex: texture_2d<f32>;

struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: f32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) control_uv: vec2<f32>,
    @location(1) splat_uv: vec2<f32>,
};

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    var out: VertexOutput;

    let sx = (in.position.x - u.camera_center.x) * u.zoom;
    let sy = (in.position.y - u.camera_center.y) * u.zoom;
    let ndc_x = sx / (u.screen_size.x * 0.5);
    let ndc_y = sy / (u.screen_size.y * 0.5);
    out.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);

    out.control_uv = vec2<f32>(in.uv.y * u.inv_control_size + u.inv_control_size_half, 0.0);
    out.splat_uv = vec2<f32>(in.uv.x * u.splat_params_x, in.color);

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let splat1 = textureSample(splat1_tex, repeat_sampler, in.splat_uv);
    let selector = floor(textureSample(control_tex, clamp_sampler, in.control_uv).y);
    var result = textureSample(splat0_tex, repeat_sampler, in.splat_uv);
    result = vec4<f32>(result.xyz + (splat1.xyz - result.xyz) * selector, result.w);
    return result;
}