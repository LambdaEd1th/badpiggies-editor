//! Small immediate 2D command layer rendered directly by wgpu.
//!
//! The level renderer historically emitted GUI paint primitives around its
//! custom GPU passes. This module owns those primitives, textures, input state,
//! and the final render pass so the canvas has no GUI-toolkit dependency.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};
use std::rc::Rc;

use bytemuck::{Pod, Zeroable};
#[cfg(target_arch = "wasm32")]
use js_sys::{Reflect, Uint8Array};
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(inline_js = r#"
let systemTextCanvas = null;
let systemTextContext = null;

function ensureSystemTextContext() {
    if (systemTextContext) return systemTextContext;
    if (typeof document !== "undefined" && typeof document.createElement === "function") {
        systemTextCanvas = document.createElement("canvas");
    } else if (typeof OffscreenCanvas !== "undefined") {
        systemTextCanvas = new OffscreenCanvas(1, 1);
    } else {
        throw new Error("Canvas2D is unavailable for system font rasterization");
    }
    systemTextContext = systemTextCanvas.getContext("2d", { willReadFrequently: true });
    if (!systemTextContext) {
        throw new Error("Canvas2D context is unavailable for system font rasterization");
    }
    return systemTextContext;
}

function systemFont(size) {
    return `400 ${Math.max(1, Number(size) || 1)}px system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif`;
}

export function systemTextRasterizerAvailable() {
    try {
        const probe = rasterizeSystemText("Aa\u4e2d\u0639", 12);
        for (const alpha of probe.alpha) {
            if (alpha !== 0) {
                return true;
            }
        }
        return false;
    } catch (_) {
        return false;
    }
}

export function rasterizeSystemText(text, size) {
    const context = ensureSystemTextContext();
    const content = String(text);
    const fontSize = Math.max(1, Number(size) || 1);
    const font = systemFont(fontSize);
    context.font = font;
    const metrics = context.measureText(content);
    const padding = 2;
    const width = Math.max(1, Math.ceil(metrics.width) + padding * 2);
    const height = Math.max(1, Math.ceil(fontSize * 1.4) + padding * 2);

    systemTextCanvas.width = width;
    systemTextCanvas.height = height;
    context.font = font;
    context.textAlign = "left";
    context.textBaseline = "top";
    context.fillStyle = "white";
    context.clearRect(0, 0, width, height);
    context.fillText(content, padding, padding);

    const rgba = context.getImageData(0, 0, width, height).data;
    const alpha = new Uint8Array(width * height);
    for (let index = 0; index < alpha.length; index += 1) {
        alpha[index] = rgba[index * 4 + 3];
    }
    return { width, height, alpha };
}
"#)]
extern "C" {
    #[wasm_bindgen(js_name = systemTextRasterizerAvailable)]
    fn system_text_rasterizer_available() -> bool;

    #[wasm_bindgen(catch, js_name = rasterizeSystemText)]
    fn rasterize_system_text(text: &str, size: f32) -> Result<JsValue, JsValue>;
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos2 {
    pub x: f32,
    pub y: f32,
}

impl Pos2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub const fn to_vec2(self) -> Vec2 {
        Vec2 {
            x: self.x,
            y: self.y,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    pub fn length(self) -> f32 {
        self.length_sq().sqrt()
    }

    pub const fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }
}

pub const fn pos2(x: f32, y: f32) -> Pos2 {
    Pos2 { x, y }
}

pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Add<Vec2> for Pos2 {
    type Output = Pos2;
    fn add(self, rhs: Vec2) -> Self::Output {
        pos2(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign<Vec2> for Pos2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub<Vec2> for Pos2 {
    type Output = Pos2;
    fn sub(self, rhs: Vec2) -> Self::Output {
        pos2(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Sub for Pos2 {
    type Output = Vec2;
    fn sub(self, rhs: Pos2) -> Self::Output {
        vec2(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, rhs: Vec2) -> Self::Output {
        vec2(self.x + rhs.x, self.y + rhs.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, rhs: Vec2) {
        self.x += rhs.x;
        self.y += rhs.y;
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, rhs: Vec2) -> Self::Output {
        vec2(self.x - rhs.x, self.y - rhs.y)
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, rhs: Vec2) {
        self.x -= rhs.x;
        self.y -= rhs.y;
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, rhs: f32) -> Self::Output {
        vec2(self.x * rhs, self.y * rhs)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, rhs: f32) -> Self::Output {
        vec2(self.x / rhs, self.y / rhs)
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Self::Output {
        vec2(-self.x, -self.y)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min: Pos2,
    pub max: Pos2,
}

impl Rect {
    pub fn from_min_max(min: Pos2, max: Pos2) -> Self {
        Self { min, max }
    }

    pub fn from_two_pos(a: Pos2, b: Pos2) -> Self {
        Self {
            min: pos2(a.x.min(b.x), a.y.min(b.y)),
            max: pos2(a.x.max(b.x), a.y.max(b.y)),
        }
    }

    pub fn from_center_size(center: Pos2, size: Vec2) -> Self {
        let half = size * 0.5;
        Self::from_min_max(center - half, center + half)
    }

    pub fn from_min_size(min: Pos2, size: Vec2) -> Self {
        Self::from_min_max(min, min + size)
    }

    pub fn left(self) -> f32 {
        self.min.x
    }
    pub fn right(self) -> f32 {
        self.max.x
    }
    pub fn top(self) -> f32 {
        self.min.y
    }
    pub fn bottom(self) -> f32 {
        self.max.y
    }
    pub fn width(self) -> f32 {
        self.max.x - self.min.x
    }
    pub fn height(self) -> f32 {
        self.max.y - self.min.y
    }
    pub fn center(self) -> Pos2 {
        pos2(
            (self.min.x + self.max.x) * 0.5,
            (self.min.y + self.max.y) * 0.5,
        )
    }
    pub fn left_top(self) -> Pos2 {
        self.min
    }
    pub fn right_top(self) -> Pos2 {
        pos2(self.max.x, self.min.y)
    }
    pub fn left_bottom(self) -> Pos2 {
        pos2(self.min.x, self.max.y)
    }
    pub fn right_bottom(self) -> Pos2 {
        self.max
    }
    pub fn contains(self, point: Pos2) -> bool {
        point.x >= self.min.x
            && point.x <= self.max.x
            && point.y >= self.min.y
            && point.y <= self.max.y
    }
    pub fn intersects(self, other: Self) -> bool {
        self.min.x <= other.max.x
            && self.max.x >= other.min.x
            && self.min.y <= other.max.y
            && self.max.y >= other.min.y
    }
    pub fn intersect(self, other: Self) -> Self {
        Self::from_min_max(
            pos2(self.min.x.max(other.min.x), self.min.y.max(other.min.y)),
            pos2(self.max.x.min(other.max.x), self.max.y.min(other.max.y)),
        )
    }
    pub fn expand(self, amount: f32) -> Self {
        Self::from_min_max(
            pos2(self.min.x - amount, self.min.y - amount),
            pos2(self.max.x + amount, self.max.y + amount),
        )
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Color32([u8; 4]);

impl Color32 {
    pub const TRANSPARENT: Self = Self([0, 0, 0, 0]);
    pub const BLACK: Self = Self([0, 0, 0, 255]);
    pub const WHITE: Self = Self([255, 255, 255, 255]);
    pub const YELLOW: Self = Self([255, 255, 0, 255]);

    pub const fn from_rgb(r: u8, g: u8, b: u8) -> Self {
        Self([r, g, b, 255])
    }

    pub fn from_rgba_unmultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        let premultiply = |value: u8| ((u16::from(value) * u16::from(a) + 127) / 255) as u8;
        Self([premultiply(r), premultiply(g), premultiply(b), a])
    }

    pub const fn from_rgba_premultiplied(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self([r, g, b, a])
    }

    pub const fn r(self) -> u8 {
        self.0[0]
    }
    pub const fn g(self) -> u8 {
        self.0[1]
    }
    pub const fn b(self) -> u8 {
        self.0[2]
    }
    pub const fn a(self) -> u8 {
        self.0[3]
    }
    pub const fn to_array(self) -> [u8; 4] {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Stroke {
    pub width: f32,
    pub color: Color32,
}

impl Stroke {
    pub const NONE: Self = Self {
        width: 0.0,
        color: Color32::TRANSPARENT,
    };

    pub const fn new(width: f32, color: Color32) -> Self {
        Self { width, color }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum StrokeKind {
    Inside,
    Outside,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TextureId(pub usize);

#[derive(Clone, Debug)]
pub struct Vertex {
    pub pos: Pos2,
    pub uv: Pos2,
    pub color: Color32,
}

pub mod epaint {
    pub use super::Vertex;
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub indices: Vec<u32>,
    pub vertices: Vec<Vertex>,
    pub texture_id: TextureId,
}

impl Mesh {
    pub fn with_texture(texture_id: TextureId) -> Self {
        Self {
            texture_id,
            ..Self::default()
        }
    }

    pub fn add_rect_with_uv(&mut self, rect: Rect, uv: Rect, color: Color32) {
        let base = self.vertices.len() as u32;
        self.vertices.extend_from_slice(&[
            Vertex {
                pos: rect.left_top(),
                uv: uv.left_top(),
                color,
            },
            Vertex {
                pos: rect.right_top(),
                uv: uv.right_top(),
                color,
            },
            Vertex {
                pos: rect.right_bottom(),
                uv: uv.right_bottom(),
                color,
            },
            Vertex {
                pos: rect.left_bottom(),
                uv: uv.left_bottom(),
                color,
            },
        ]);
        self.indices
            .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
    }

    pub fn is_valid(&self) -> bool {
        self.indices
            .iter()
            .all(|&index| index < self.vertices.len() as u32)
    }

    fn append(&mut self, source: Mesh) {
        debug_assert_eq!(self.texture_id, source.texture_id);
        let base = self.vertices.len() as u32;
        self.vertices.extend(source.vertices);
        self.indices
            .extend(source.indices.into_iter().map(|index| index + base));
    }
}

pub trait PaintCallback {
    fn prepare(&self, queue: &wgpu::Queue);
    fn paint(&self, render_pass: &mut wgpu::RenderPass<'static>);
}

#[derive(Clone)]
pub struct Callback {
    pub clip_rect: Rect,
    pub callback: Rc<dyn PaintCallback>,
}

impl Callback {
    pub fn new(clip_rect: Rect, callback: impl PaintCallback + 'static) -> Self {
        Self {
            clip_rect,
            callback: Rc::new(callback),
        }
    }
}

#[derive(Clone)]
pub enum Shape {
    Mesh(Mesh),
    Callback(Callback),
}

impl Shape {
    pub fn mesh(mesh: Mesh) -> Self {
        Self::Mesh(mesh)
    }

    pub fn line(points: Vec<Pos2>, stroke: Stroke) -> Self {
        Self::Mesh(polyline_mesh(&points, stroke, false))
    }

    pub fn convex_polygon(points: Vec<Pos2>, fill: Color32, stroke: Stroke) -> Self {
        let mut mesh = Mesh::default();
        if points.len() >= 3 && fill.a() > 0 {
            mesh.vertices
                .extend(points.iter().copied().map(|pos| Vertex {
                    pos,
                    uv: Pos2::ZERO,
                    color: fill,
                }));
            for index in 1..points.len() - 1 {
                mesh.indices
                    .extend_from_slice(&[0, index as u32, index as u32 + 1]);
            }
        }
        append_mesh(&mut mesh, polyline_mesh(&points, stroke, true));
        Self::Mesh(mesh)
    }
}

fn append_mesh(destination: &mut Mesh, source: Mesh) {
    destination.append(source);
}

fn segment_mesh(start: Pos2, end: Pos2, stroke: Stroke) -> Mesh {
    let mut mesh = Mesh::default();
    append_segment(&mut mesh, start, end, stroke);
    mesh
}

fn append_segment(mesh: &mut Mesh, start: Pos2, end: Pos2, stroke: Stroke) {
    if stroke.width <= 0.0 || stroke.color.a() == 0 {
        return;
    }
    let delta = end - start;
    let length = delta.length();
    if length <= f32::EPSILON {
        append_mesh(
            mesh,
            circle_mesh(start, stroke.width * 0.5, stroke.color, false, Stroke::NONE),
        );
        return;
    }
    let normal = vec2(-delta.y / length, delta.x / length) * (stroke.width * 0.5);
    let base = mesh.vertices.len() as u32;
    mesh.vertices.extend_from_slice(&[
        Vertex {
            pos: start + normal,
            uv: Pos2::ZERO,
            color: stroke.color,
        },
        Vertex {
            pos: end + normal,
            uv: Pos2::ZERO,
            color: stroke.color,
        },
        Vertex {
            pos: end - normal,
            uv: Pos2::ZERO,
            color: stroke.color,
        },
        Vertex {
            pos: start - normal,
            uv: Pos2::ZERO,
            color: stroke.color,
        },
    ]);
    mesh.indices
        .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn polyline_mesh(points: &[Pos2], stroke: Stroke, closed: bool) -> Mesh {
    let mut mesh = Mesh::default();
    for pair in points.windows(2) {
        append_segment(&mut mesh, pair[0], pair[1], stroke);
    }
    if closed && points.len() > 2 {
        append_segment(
            &mut mesh,
            *points.last().unwrap_or(&Pos2::ZERO),
            points[0],
            stroke,
        );
    }
    mesh
}

fn circle_mesh(center: Pos2, radius: f32, fill: Color32, filled: bool, stroke: Stroke) -> Mesh {
    let segments = ((radius * 0.75).ceil() as usize).clamp(12, 64);
    let points: Vec<_> = (0..segments)
        .map(|index| {
            let angle = std::f32::consts::TAU * index as f32 / segments as f32;
            pos2(
                center.x + radius * angle.cos(),
                center.y + radius * angle.sin(),
            )
        })
        .collect();
    let mut mesh = Mesh::default();
    if filled && fill.a() > 0 {
        mesh.vertices.push(Vertex {
            pos: center,
            uv: Pos2::ZERO,
            color: fill,
        });
        mesh.vertices
            .extend(points.iter().copied().map(|pos| Vertex {
                pos,
                uv: Pos2::ZERO,
                color: fill,
            }));
        for index in 0..segments {
            mesh.indices.extend_from_slice(&[
                0,
                index as u32 + 1,
                ((index + 1) % segments) as u32 + 1,
            ]);
        }
    }
    append_mesh(&mut mesh, polyline_mesh(&points, stroke, true));
    mesh
}

#[derive(Clone, Copy, Debug)]
pub struct FontId {
    pub size: f32,
}

impl FontId {
    pub const fn proportional(size: f32) -> Self {
        Self { size }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Align2(u8);

impl Align2 {
    pub const LEFT_TOP: Self = Self(0);
    pub const RIGHT_TOP: Self = Self(1);
    pub const CENTER_TOP: Self = Self(2);
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TextureOptions {
    pub wrap_mode: TextureWrapMode,
}

impl TextureOptions {
    pub const LINEAR: Self = Self {
        wrap_mode: TextureWrapMode::ClampToEdge,
    };
}

#[derive(Clone, Copy, Debug, Default)]
pub enum TextureWrapMode {
    #[default]
    ClampToEdge,
    Repeat,
}

#[derive(Clone)]
pub struct ColorImage {
    pub size: [usize; 2],
    pub pixels: Vec<Color32>,
}

impl ColorImage {
    pub fn new(size: [usize; 2], pixels: Vec<Color32>) -> Self {
        Self { size, pixels }
    }
}

struct TextureResource {
    _texture: wgpu::Texture,
    bind_group: wgpu::BindGroup,
}

struct ContextInner {
    device: wgpu::Device,
    queue: wgpu::Queue,
    texture_layout: wgpu::BindGroupLayout,
    textures: RefCell<Vec<TextureResource>>,
    system_font_available: bool,
    #[cfg(not(target_arch = "wasm32"))]
    native_text: RefCell<Option<crate::native_text::NativeTextRasterizer>>,
    text_cache: RefCell<HashMap<(String, u32), TextureHandle>>,
    cursor: Cell<CursorIcon>,
    input: RefCell<InputState>,
    repaint_requested: Cell<bool>,
}

#[derive(Clone)]
pub struct Context(Rc<ContextInner>);

impl Context {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Self {
        let texture_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("gpu2d_texture_layout"),
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
        #[cfg(target_arch = "wasm32")]
        let system_font_available = system_text_rasterizer_available();
        #[cfg(not(target_arch = "wasm32"))]
        let native_text = crate::native_text::NativeTextRasterizer::new();
        #[cfg(not(target_arch = "wasm32"))]
        let system_font_available = native_text.is_some();
        if !system_font_available {
            log::warn!("System font rasterizer is unavailable");
        }
        let context = Self(Rc::new(ContextInner {
            device: device.clone(),
            queue: queue.clone(),
            texture_layout,
            textures: RefCell::new(Vec::new()),
            system_font_available,
            #[cfg(not(target_arch = "wasm32"))]
            native_text: RefCell::new(native_text),
            text_cache: RefCell::new(HashMap::new()),
            cursor: Cell::new(CursorIcon::Default),
            input: RefCell::new(InputState::default()),
            repaint_requested: Cell::new(true),
        }));
        context.load_texture(
            "gpu2d_white",
            ColorImage::new([1, 1], vec![Color32::WHITE]),
            TextureOptions::LINEAR,
        );
        context
    }

    pub fn load_texture(
        &self,
        _name: impl Into<String>,
        image: ColorImage,
        options: TextureOptions,
    ) -> TextureHandle {
        let width = image.size[0].max(1) as u32;
        let height = image.size[1].max(1) as u32;
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.0.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("gpu2d_texture"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let pixels: Vec<u8> = image
            .pixels
            .iter()
            .flat_map(|color| color.to_array())
            .collect();
        self.0.queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &pixels,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            size,
        );
        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let address_mode = match options.wrap_mode {
            TextureWrapMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            TextureWrapMode::Repeat => wgpu::AddressMode::Repeat,
        };
        let sampler = self.0.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("gpu2d_sampler"),
            address_mode_u: address_mode,
            address_mode_v: address_mode,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bind_group = self.0.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("gpu2d_texture_bind_group"),
            layout: &self.0.texture_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let mut textures = self.0.textures.borrow_mut();
        let handle = TextureHandle {
            id: TextureId(textures.len()),
            size: image.size,
        };
        textures.push(TextureResource {
            _texture: texture,
            bind_group,
        });
        handle
    }

    pub fn set_cursor_icon(&self, cursor: CursorIcon) {
        self.0.cursor.set(cursor);
    }

    pub fn cursor_icon(&self) -> CursorIcon {
        self.0.cursor.get()
    }

    pub fn reset_cursor_icon(&self) {
        self.0.cursor.set(CursorIcon::Default);
    }

    pub fn request_repaint(&self) {
        self.0.repaint_requested.set(true);
    }

    pub fn take_repaint_request(&self) -> bool {
        self.0.repaint_requested.replace(false)
    }

    pub fn repaint_requested(&self) -> bool {
        self.0.repaint_requested.get()
    }

    pub fn font_backend(&self) -> &'static str {
        if self.0.system_font_available {
            if cfg!(target_arch = "wasm32") {
                "canvas2d-system"
            } else {
                "cosmic-text-system"
            }
        } else {
            "unavailable"
        }
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.0.input.borrow())
    }

    fn text_texture(&self, text: &str, size: f32) -> Option<TextureHandle> {
        let key = (text.to_string(), (size.max(1.0) * 64.0).round() as u32);
        if let Some(handle) = self.0.text_cache.borrow().get(&key) {
            return Some(handle.clone());
        }
        if !self.0.system_font_available {
            return None;
        }
        #[cfg(target_arch = "wasm32")]
        let (width, height, alpha) = {
            let rasterized = rasterize_system_text(text, size).ok()?;
            let width = Reflect::get(&rasterized, &JsValue::from_str("width"))
                .ok()?
                .as_f64()? as usize;
            let height = Reflect::get(&rasterized, &JsValue::from_str("height"))
                .ok()?
                .as_f64()? as usize;
            let alpha =
                Uint8Array::new(&Reflect::get(&rasterized, &JsValue::from_str("alpha")).ok()?)
                    .to_vec();
            (width, height, alpha)
        };
        #[cfg(not(target_arch = "wasm32"))]
        let (width, height, alpha) = {
            let rasterized = self
                .0
                .native_text
                .borrow_mut()
                .as_mut()?
                .rasterize(text, size)?;
            (rasterized.width, rasterized.height, rasterized.alpha)
        };
        if width == 0 || height == 0 || alpha.len() != width * height {
            return None;
        }
        let pixels = alpha
            .into_iter()
            .map(|alpha| Color32::from_rgba_premultiplied(alpha, alpha, alpha, alpha))
            .collect();
        let handle = self.load_texture(
            format!("gpu2d_text_{}_{}", key.1, text),
            ColorImage::new([width, height], pixels),
            TextureOptions::LINEAR,
        );
        self.0.text_cache.borrow_mut().insert(key, handle.clone());
        Some(handle)
    }
}

#[derive(Clone)]
pub struct TextureHandle {
    id: TextureId,
    size: [usize; 2],
}

impl TextureHandle {
    pub const fn id(&self) -> TextureId {
        self.id
    }
    pub const fn size(&self) -> [usize; 2] {
        self.size
    }
}

#[derive(Clone)]
pub struct Painter {
    commands: Rc<RefCell<Vec<Shape>>>,
    context: Context,
}

impl Painter {
    pub fn new(context: Context) -> Self {
        Self {
            commands: Rc::new(RefCell::new(Vec::new())),
            context,
        }
    }

    pub fn add(&self, shape: Shape) {
        let mut commands = self.commands.borrow_mut();
        match shape {
            Shape::Mesh(mesh) => {
                if let Some(Shape::Mesh(previous)) = commands.last_mut()
                    && previous.texture_id == mesh.texture_id
                {
                    previous.append(mesh);
                    return;
                }
                commands.push(Shape::Mesh(mesh));
            }
            Shape::Callback(callback) => commands.push(Shape::Callback(callback)),
        }
    }

    pub fn line_segment(&self, points: [Pos2; 2], stroke: Stroke) {
        self.add(Shape::Mesh(segment_mesh(points[0], points[1], stroke)));
    }

    pub fn rect_filled(&self, rect: Rect, _rounding: f32, fill: Color32) {
        let mut mesh = Mesh::default();
        mesh.add_rect_with_uv(rect, Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)), fill);
        self.add(Shape::Mesh(mesh));
    }

    pub fn rect_stroke(&self, rect: Rect, _rounding: f32, stroke: Stroke, _kind: StrokeKind) {
        self.add(Shape::Mesh(polyline_mesh(
            &[
                rect.left_top(),
                rect.right_top(),
                rect.right_bottom(),
                rect.left_bottom(),
            ],
            stroke,
            true,
        )));
    }

    pub fn rect(&self, rect: Rect, rounding: f32, fill: Color32, stroke: Stroke, kind: StrokeKind) {
        self.rect_filled(rect, rounding, fill);
        self.rect_stroke(rect, rounding, stroke, kind);
    }

    pub fn circle_filled(&self, center: Pos2, radius: f32, fill: Color32) {
        self.add(Shape::Mesh(circle_mesh(
            center,
            radius,
            fill,
            true,
            Stroke::NONE,
        )));
    }

    pub fn circle_stroke(&self, center: Pos2, radius: f32, stroke: Stroke) {
        self.add(Shape::Mesh(circle_mesh(
            center,
            radius,
            Color32::TRANSPARENT,
            false,
            stroke,
        )));
    }

    pub fn text(
        &self,
        position: Pos2,
        align: Align2,
        text: impl ToString,
        font: FontId,
        color: Color32,
    ) {
        let text = text.to_string();
        let Some(texture) = self.context.text_texture(&text, font.size) else {
            return;
        };
        let size = vec2(texture.size[0] as f32, texture.size[1] as f32);
        let min = if align == Align2::RIGHT_TOP {
            position - vec2(size.x, 0.0)
        } else if align == Align2::CENTER_TOP {
            position - vec2(size.x * 0.5, 0.0)
        } else {
            position
        };
        let mut mesh = Mesh::with_texture(texture.id());
        mesh.add_rect_with_uv(
            Rect::from_min_size(min, size),
            Rect::from_min_max(Pos2::ZERO, pos2(1.0, 1.0)),
            color,
        );
        self.add(Shape::Mesh(mesh));
    }

    pub fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn take_commands(&self) -> Vec<Shape> {
        std::mem::take(&mut *self.commands.borrow_mut())
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PointerButton {
    #[default]
    Primary,
    Secondary,
    Middle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Key {
    Enter,
    Escape,
    Delete,
    Backspace,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Modifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    pub command: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct TouchTransform {
    pub zoom_delta: f32,
    pub translation_delta: Vec2,
    pub center: Pos2,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct PointerState {
    pub position: Option<Pos2>,
}

impl PointerState {
    pub const fn latest_pos(&self) -> Option<Pos2> {
        self.position
    }
}

#[derive(Clone, Debug)]
pub struct InputState {
    pub stable_dt: f32,
    pub modifiers: Modifiers,
    pub smooth_scroll_delta: Vec2,
    pub pointer: PointerState,
    pub touch_transforms: Vec<TouchTransform>,
    pub keys_pressed: HashSet<Key>,
}

impl Default for InputState {
    fn default() -> Self {
        Self {
            stable_dt: 1.0 / 60.0,
            modifiers: Modifiers::default(),
            smooth_scroll_delta: Vec2::ZERO,
            pointer: PointerState::default(),
            touch_transforms: Vec::new(),
            keys_pressed: HashSet::new(),
        }
    }
}

impl InputState {
    pub fn key_pressed(&self, key: Key) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn touch_transforms(&self) -> &[TouchTransform] {
        &self.touch_transforms
    }
}

#[derive(Clone, Debug, Default)]
pub struct Response {
    pub rect: Rect,
    pub pointer_pos: Option<Pos2>,
    pub hovered: bool,
    pub drag_delta: Vec2,
    pub buttons_down: HashSet<PointerButton>,
    pub buttons_pressed: HashSet<PointerButton>,
    pub buttons_released: HashSet<PointerButton>,
    pub buttons_clicked: HashSet<PointerButton>,
    pub double_clicked: bool,
}

impl Response {
    pub const fn hover_pos(&self) -> Option<Pos2> {
        if self.hovered { self.pointer_pos } else { None }
    }
    pub const fn interact_pointer_pos(&self) -> Option<Pos2> {
        self.pointer_pos
    }
    pub const fn hovered(&self) -> bool {
        self.hovered
    }
    pub fn clicked(&self) -> bool {
        self.buttons_clicked.contains(&PointerButton::Primary)
    }
    pub fn secondary_clicked(&self) -> bool {
        self.buttons_clicked.contains(&PointerButton::Secondary)
    }
    pub const fn double_clicked(&self) -> bool {
        self.double_clicked
    }
    pub fn dragged_by(&self, button: PointerButton) -> bool {
        self.buttons_down.contains(&button)
    }
    pub fn drag_started_by(&self, button: PointerButton) -> bool {
        self.buttons_pressed.contains(&button)
    }
    pub fn drag_stopped_by(&self, button: PointerButton) -> bool {
        self.buttons_released.contains(&button)
    }
    pub const fn drag_delta(&self) -> Vec2 {
        self.drag_delta
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Sense;

impl Sense {
    pub const fn click_and_drag() -> Self {
        Self
    }
}

pub struct Ui {
    size: Vec2,
    context: Context,
    input: InputState,
    response: Response,
    painter: Painter,
}

impl Ui {
    pub fn new(context: Context, size: Vec2, input: InputState, response: Response) -> Self {
        *context.0.input.borrow_mut() = input.clone();
        let painter = Painter::new(context.clone());
        Self {
            size,
            context,
            input,
            response,
            painter,
        }
    }

    pub const fn available_size(&self) -> Vec2 {
        self.size
    }

    pub fn allocate_painter(&mut self, _size: Vec2, _sense: Sense) -> (Response, Painter) {
        (self.response.clone(), self.painter.clone())
    }

    pub fn input<R>(&self, reader: impl FnOnce(&InputState) -> R) -> R {
        reader(&self.input)
    }

    pub const fn ctx(&self) -> &Context {
        &self.context
    }

    pub fn painter(&self) -> &Painter {
        &self.painter
    }

    pub fn max_rect(&self) -> Rect {
        Rect::from_min_size(Pos2::ZERO, self.size)
    }

    pub fn take_commands(&self) -> Vec<Shape> {
        self.painter.take_commands()
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum CursorIcon {
    #[default]
    Default,
    Grab,
    Grabbing,
    ResizeHorizontal,
    ResizeVertical,
    ResizeNwSe,
    ResizeNeSw,
}

impl CursorIcon {
    pub const fn css(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::Grab => "grab",
            Self::Grabbing => "grabbing",
            Self::ResizeHorizontal => "ew-resize",
            Self::ResizeVertical => "ns-resize",
            Self::ResizeNwSe => "nwse-resize",
            Self::ResizeNeSw => "nesw-resize",
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct GpuVertex {
    position: [f32; 2],
    uv: [f32; 2],
    color: [u8; 4],
}

const INITIAL_STREAM_BUFFER_SIZE: u64 = 64 * 1024;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct FrameStats {
    pub frame_number: u64,
    pub input_shapes: u32,
    pub mesh_batches: u32,
    pub callback_count: u32,
    pub draw_calls: u32,
    pub vertices: u32,
    pub indices: u32,
    pub culled_shapes: u32,
    pub buffer_creations: u32,
}

enum PreparedItem {
    Mesh {
        texture_id: TextureId,
        index_start: u32,
        index_count: u32,
    },
    Callback(Callback),
}

#[derive(Default)]
struct RenderList {
    vertices: Vec<GpuVertex>,
    indices: Vec<u32>,
    items: Vec<PreparedItem>,
}

impl RenderList {
    fn clear(&mut self) {
        self.vertices.clear();
        self.indices.clear();
        self.items.clear();
    }
}

pub struct Renderer {
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_capacity: u64,
    render_list: RenderList,
    frame_number: u64,
    last_stats: FrameStats,
}

fn egui_premultiplied_alpha_blending() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::OneMinusDstAlpha,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct RenderViewport {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

const VIEWPORT_CORNER_SEGMENTS: usize = 32;

fn normalized_color_channel(channel: f64) -> u8 {
    (channel.clamp(0.0, 1.0) * 255.0).round() as u8
}

fn bottom_corner_mask(width: f32, height: f32, radius: f32, color: Color32) -> Mesh {
    let radius = radius.min(width * 0.5).min(height).max(0.0);
    if radius <= 0.0 || !radius.is_finite() {
        return Mesh::default();
    }

    let mut mesh = Mesh::default();
    append_corner_mask_fan(
        &mut mesh,
        pos2(0.0, height),
        pos2(radius, height - radius),
        radius,
        std::f32::consts::PI,
        std::f32::consts::FRAC_PI_2,
        color,
    );
    append_corner_mask_fan(
        &mut mesh,
        pos2(width, height),
        pos2(width - radius, height - radius),
        radius,
        std::f32::consts::FRAC_PI_2,
        0.0,
        color,
    );
    mesh
}

fn append_corner_mask_fan(
    mesh: &mut Mesh,
    anchor: Pos2,
    center: Pos2,
    radius: f32,
    start_angle: f32,
    end_angle: f32,
    color: Color32,
) {
    let base = mesh.vertices.len() as u32;
    mesh.vertices.push(Vertex {
        pos: anchor,
        uv: Pos2::ZERO,
        color,
    });
    for step in 0..=VIEWPORT_CORNER_SEGMENTS {
        let progress = step as f32 / VIEWPORT_CORNER_SEGMENTS as f32;
        let angle = start_angle + (end_angle - start_angle) * progress;
        mesh.vertices.push(Vertex {
            pos: pos2(
                center.x + angle.cos() * radius,
                center.y + angle.sin() * radius,
            ),
            uv: Pos2::ZERO,
            color,
        });
    }
    for step in 0..VIEWPORT_CORNER_SEGMENTS as u32 {
        mesh.indices
            .extend_from_slice(&[base, base + step + 1, base + step + 2]);
    }
}

impl Renderer {
    pub fn new(device: &wgpu::Device, format: wgpu::TextureFormat, context: &Context) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("gpu2d_shader"),
            source: wgpu::ShaderSource::Wgsl(
                r#"
struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var output: VertexOutput;
    output.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    output.uv = input.uv;
    output.color = input.color;
    return output;
}

@group(0) @binding(0) var texture_image: texture_2d<f32>;
@group(0) @binding(1) var texture_sampler: sampler;

fn interleaved_gradient_noise(n: vec2<f32>) -> f32 {
    let f = 0.06711056 * n.x + 0.00583715 * n.y;
    return fract(52.9829189 * fract(f));
}

fn dither_interleaved(rgb: vec3<f32>, frag_coord: vec4<f32>) -> vec3<f32> {
    var noise = interleaved_gradient_noise(frag_coord.xy);
    noise = (noise - 0.5) * 0.95;
    return rgb + noise / 255.0;
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(texture_image, texture_sampler, input.uv) * input.color;
    return vec4<f32>(dither_interleaved(color.rgb, input.clip_position), color.a);
}
"#
                .into(),
            ),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("gpu2d_pipeline_layout"),
            bind_group_layouts: &[Some(&context.0.texture_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("gpu2d_pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Some(wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<GpuVertex>() as u64,
                    step_mode: wgpu::VertexStepMode::Vertex,
                    attributes: &[
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 8,
                            shader_location: 1,
                        },
                        wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Unorm8x4,
                            offset: 16,
                            shader_location: 2,
                        },
                    ],
                })],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(egui_premultiplied_alpha_blending()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });
        let vertex_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu2d_stream_vertices"),
            size: INITIAL_STREAM_BUFFER_SIZE,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let index_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("gpu2d_stream_indices"),
            size: INITIAL_STREAM_BUFFER_SIZE,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            vertex_buffer,
            index_buffer,
            vertex_capacity: INITIAL_STREAM_BUFFER_SIZE,
            index_capacity: INITIAL_STREAM_BUFFER_SIZE,
            render_list: RenderList::default(),
            frame_number: 0,
            last_stats: FrameStats::default(),
        }
    }

    fn mesh_is_visible(mesh: &Mesh, width: u32, height: u32) -> bool {
        let Some(first) = mesh.vertices.first() else {
            return false;
        };
        let mut min_x = first.pos.x;
        let mut min_y = first.pos.y;
        let mut max_x = first.pos.x;
        let mut max_y = first.pos.y;
        for vertex in &mesh.vertices[1..] {
            min_x = min_x.min(vertex.pos.x);
            min_y = min_y.min(vertex.pos.y);
            max_x = max_x.max(vertex.pos.x);
            max_y = max_y.max(vertex.pos.y);
        }
        max_x >= 0.0 && max_y >= 0.0 && min_x <= width as f32 && min_y <= height as f32
    }

    fn prepare_render_list(&mut self, commands: Vec<Shape>, width: u32, height: u32) -> FrameStats {
        self.render_list.clear();

        let mut stats = FrameStats {
            input_shapes: commands.len() as u32,
            ..FrameStats::default()
        };
        let width_f = width.max(1) as f32;
        let height_f = height.max(1) as f32;

        for command in commands {
            match command {
                Shape::Mesh(mesh) if !mesh.vertices.is_empty() && !mesh.indices.is_empty() => {
                    if !Self::mesh_is_visible(&mesh, width, height) {
                        stats.culled_shapes += 1;
                        continue;
                    }
                    let vertex_base = self.render_list.vertices.len() as u32;
                    let index_start = self.render_list.indices.len() as u32;
                    self.render_list
                        .vertices
                        .extend(mesh.vertices.iter().map(|vertex| GpuVertex {
                            position: [
                                vertex.pos.x / width_f * 2.0 - 1.0,
                                1.0 - vertex.pos.y / height_f * 2.0,
                            ],
                            uv: [vertex.uv.x, vertex.uv.y],
                            color: vertex.color.to_array(),
                        }));
                    self.render_list
                        .indices
                        .extend(mesh.indices.iter().map(|index| index + vertex_base));
                    let index_count = self.render_list.indices.len() as u32 - index_start;

                    if let Some(PreparedItem::Mesh {
                        texture_id,
                        index_count: previous_count,
                        ..
                    }) = self.render_list.items.last_mut()
                        && *texture_id == mesh.texture_id
                    {
                        *previous_count += index_count;
                    } else {
                        self.render_list.items.push(PreparedItem::Mesh {
                            texture_id: mesh.texture_id,
                            index_start,
                            index_count,
                        });
                        stats.mesh_batches += 1;
                    }
                }
                Shape::Callback(callback) => {
                    self.render_list
                        .items
                        .push(PreparedItem::Callback(callback));
                    stats.callback_count += 1;
                }
                Shape::Mesh(_) => {}
            }
        }

        stats.vertices = self.render_list.vertices.len() as u32;
        stats.indices = self.render_list.indices.len() as u32;
        stats.draw_calls = stats.mesh_batches + stats.callback_count;
        stats
    }

    fn ensure_buffer_capacity(
        device: &wgpu::Device,
        buffer: &mut wgpu::Buffer,
        capacity: &mut u64,
        required: u64,
        usage: wgpu::BufferUsages,
        label: &'static str,
    ) -> bool {
        if required <= *capacity {
            return false;
        }
        *capacity = required.max(INITIAL_STREAM_BUFFER_SIZE).next_power_of_two();
        *buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size: *capacity,
            usage: usage | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        true
    }

    #[cfg(target_arch = "wasm32")]
    pub const fn last_stats(&self) -> FrameStats {
        self.last_stats
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(target_arch = "wasm32")]
    pub fn render(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        context: &Context,
        commands: Vec<Shape>,
        width: u32,
        height: u32,
    ) {
        self.render_in_viewport(
            device,
            queue,
            target,
            context,
            commands,
            width,
            height,
            RenderViewport {
                x: 0,
                y: 0,
                width,
                height,
            },
            wgpu::Color {
                r: 24.0 / 255.0,
                g: 27.0 / 255.0,
                b: 32.0 / 255.0,
                a: 1.0,
            },
            0.0,
        );
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_in_viewport(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        target: &wgpu::TextureView,
        context: &Context,
        commands: Vec<Shape>,
        logical_width: u32,
        logical_height: u32,
        viewport: RenderViewport,
        backdrop_color: wgpu::Color,
        corner_radius: f32,
    ) {
        let logical_width = logical_width.max(1);
        let logical_height = logical_height.max(1);
        let viewport = RenderViewport {
            width: viewport.width.max(1),
            height: viewport.height.max(1),
            ..viewport
        };
        let mut commands = commands;
        if corner_radius > 0.0 {
            commands.push(Shape::Mesh(bottom_corner_mask(
                logical_width as f32,
                logical_height as f32,
                corner_radius,
                Color32::from_rgb(
                    normalized_color_channel(backdrop_color.r),
                    normalized_color_channel(backdrop_color.g),
                    normalized_color_channel(backdrop_color.b),
                ),
            )));
        }
        let mut stats = self.prepare_render_list(commands, logical_width, logical_height);
        self.frame_number = self.frame_number.wrapping_add(1);
        stats.frame_number = self.frame_number;
        for item in &self.render_list.items {
            if let PreparedItem::Callback(callback) = item {
                callback.callback.prepare(queue);
            }
        }

        let vertex_bytes = bytemuck::cast_slice(&self.render_list.vertices);
        let index_bytes = bytemuck::cast_slice(&self.render_list.indices);
        stats.buffer_creations += u32::from(Self::ensure_buffer_capacity(
            device,
            &mut self.vertex_buffer,
            &mut self.vertex_capacity,
            vertex_bytes.len() as u64,
            wgpu::BufferUsages::VERTEX,
            "gpu2d_stream_vertices",
        ));
        stats.buffer_creations += u32::from(Self::ensure_buffer_capacity(
            device,
            &mut self.index_buffer,
            &mut self.index_capacity,
            index_bytes.len() as u64,
            wgpu::BufferUsages::INDEX,
            "gpu2d_stream_indices",
        ));
        if !vertex_bytes.is_empty() {
            queue.write_buffer(&self.vertex_buffer, 0, vertex_bytes);
        }
        if !index_bytes.is_empty() {
            queue.write_buffer(&self.index_buffer, 0, index_bytes);
        }

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("gpu2d_encoder"),
        });
        {
            let pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("gpu2d_main_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(backdrop_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let mut pass = pass.forget_lifetime();
            pass.set_viewport(
                viewport.x as f32,
                viewport.y as f32,
                viewport.width as f32,
                viewport.height as f32,
                0.0,
                1.0,
            );
            let textures = context.0.textures.borrow();
            for item in &self.render_list.items {
                match item {
                    PreparedItem::Mesh {
                        texture_id,
                        index_start,
                        index_count,
                    } => {
                        let texture = textures.get(texture_id.0).or_else(|| textures.first());
                        let Some(texture) = texture else {
                            continue;
                        };
                        pass.set_scissor_rect(
                            viewport.x,
                            viewport.y,
                            viewport.width,
                            viewport.height,
                        );
                        pass.set_pipeline(&self.pipeline);
                        pass.set_bind_group(0, &texture.bind_group, &[]);
                        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
                        pass.set_index_buffer(
                            self.index_buffer.slice(..),
                            wgpu::IndexFormat::Uint32,
                        );
                        pass.draw_indexed(*index_start..(*index_start + *index_count), 0, 0..1);
                    }
                    PreparedItem::Callback(callback) => {
                        let scale_x = viewport.width as f32 / logical_width as f32;
                        let scale_y = viewport.height as f32 / logical_height as f32;
                        let left = viewport.x
                            + (callback.clip_rect.left().max(0.0) * scale_x).floor() as u32;
                        let top = viewport.y
                            + (callback.clip_rect.top().max(0.0) * scale_y).floor() as u32;
                        let right = viewport.x
                            + (callback.clip_rect.right().min(logical_width as f32) * scale_x)
                                .ceil() as u32;
                        let bottom = viewport.y
                            + (callback.clip_rect.bottom().min(logical_height as f32) * scale_y)
                                .ceil() as u32;
                        if right > left && bottom > top {
                            pass.set_scissor_rect(left, top, right - left, bottom - top);
                            callback.callback.paint(&mut pass);
                        }
                    }
                }
            }
        }
        queue.submit(Some(encoder.finish()));
        self.last_stats = stats;
    }
}

#[cfg(test)]
mod viewport_corner_mask_tests {
    use super::*;

    #[test]
    fn bottom_corner_mask_covers_both_corners_inside_viewport_bounds() {
        let mesh = bottom_corner_mask(800.0, 500.0, 24.0, Color32::BLACK);

        assert!(mesh.is_valid());
        assert_eq!(mesh.indices.len(), VIEWPORT_CORNER_SEGMENTS * 2 * 3);
        assert!(mesh.vertices.iter().all(|vertex| {
            vertex.pos.x >= 0.0
                && vertex.pos.x <= 800.0
                && vertex.pos.y >= 476.0
                && vertex.pos.y <= 500.0
        }));
        assert_eq!(mesh.vertices[0].pos, pos2(0.0, 500.0));
        assert!(
            mesh.vertices
                .iter()
                .any(|vertex| vertex.pos == pos2(800.0, 500.0))
        );
    }

    #[test]
    fn bottom_corner_mask_is_empty_without_rounding() {
        let mesh = bottom_corner_mask(800.0, 500.0, 0.0, Color32::BLACK);

        assert!(mesh.vertices.is_empty());
        assert!(mesh.indices.is_empty());
    }
}

#[cfg(test)]
mod color_compatibility_tests {
    use super::*;

    #[test]
    fn unmultiplied_color_matches_egui_rounding() {
        for alpha in 0..=u8::MAX {
            for value in 0..=u8::MAX {
                let expected = ((value as f32 * alpha as f32 / 255.0) + 0.5) as u8;
                let actual = Color32::from_rgba_unmultiplied(value, value, value, alpha);
                assert_eq!(actual.to_array(), [expected, expected, expected, alpha]);
            }
        }
    }

    #[test]
    fn mesh_blending_matches_egui_wgpu() {
        let blend = egui_premultiplied_alpha_blending();
        assert_eq!(blend.color.src_factor, wgpu::BlendFactor::One);
        assert_eq!(blend.color.dst_factor, wgpu::BlendFactor::OneMinusSrcAlpha);
        assert_eq!(blend.alpha.src_factor, wgpu::BlendFactor::OneMinusDstAlpha);
        assert_eq!(blend.alpha.dst_factor, wgpu::BlendFactor::One);
    }
}
