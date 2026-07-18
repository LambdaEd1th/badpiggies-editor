// WGSL runtime module for the Unity shader "Depth Mask/Unlit Transparent (CG)".
// Shared dark-overlay runtime implementation.

struct Uniforms {
    viewport_min: vec2<f32>,
    viewport_size: vec2<f32>,
    color: vec4<f32>,
    params: vec4<f32>,
    vertex_scale: vec2<f32>,
    vertex_offset: vec2<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VIn {
    @location(0) position: vec2<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) screen_uv: vec2<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let position = in.position * u.vertex_scale + u.vertex_offset;
    let local = position - u.viewport_min;
    let ndc = vec2<f32>(
        local.x / (u.viewport_size.x * 0.5) - 1.0,
        1.0 - local.y / (u.viewport_size.y * 0.5),
    );
    out.position = vec4<f32>(ndc, 0.0, 1.0);
    out.screen_uv = vec2<f32>(
        local.x / u.viewport_size.x,
        local.y / u.viewport_size.y,
    );
    return out;
}

@fragment
fn fs_color(in: VOut) -> @location(0) vec4<f32> {
    return u.color;
}

@fragment
fn fs_night_vision_overlay(in: VOut) -> @location(0) vec4<f32> {
    let centered = in.screen_uv - vec2<f32>(0.5, 0.5);
    let radius = u.params.x;
    let softness = max(u.params.y, 0.0001);
    let factor = clamp((length(centered) - radius) / (-softness), 0.0, 1.0);
    let vignette = factor * factor * (3.0 - 2.0 * factor);
    let rgb = mix(u.color.rgb, u.color.rgb * vignette, vec3<f32>(0.5, 0.5, 0.5));
    return vec4<f32>(rgb, u.color.a);
}
