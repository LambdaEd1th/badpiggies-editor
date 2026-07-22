use std::cell::{Cell, RefCell};
use std::rc::Rc;

use badpiggies_editor_core::data::runtime_assets::install_runtime_assets;
use js_sys::{Function, Reflect, Uint8Array};
use wasm_bindgen::{JsCast, JsValue, prelude::*};
use wasm_bindgen_futures::JsFuture;

use crate::ContraptionPreviewPayload;
use crate::engine::{RawInput, RendererApp, RendererEvent, ScenePayload, ViewPayload};
use crate::gpu2d;

const RENDERER_ASSETS: &[&str] = &[
    "data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage",
    "locales/en-US.ftl",
    "shader/e2d__curve.wgsl",
    "shader/_custom__unlit_color_geometry__terrain_fill.wgsl",
    "shader/unlit__transparent_cutout__sprite.wgsl",
    "shader/_custom__unlit_colortransparent_geometry__sprite.wgsl",
    "shader/_custom__unlit_monochrome.wgsl",
    "shader/_custom__unlit_color_geometry.wgsl",
    "shader/_custom__unlit_colortransparent_geometry.wgsl",
    "shader/_custom__unlit_alpha8bit_color.wgsl",
    "shader/unlit__transparent.wgsl",
    "shader/unlit__transparent_cutout.wgsl",
    "shader/depth_mask__unlit_transparent_cg__runtime.wgsl",
    "shader/depth_mask__maskoverlay__runtime.wgsl",
    "shader/depth_mask__maskoverlaynv__runtime.wgsl",
];

fn emit(event: &RendererEvent) {
    let Ok(value) = serde_wasm_bindgen::to_value(event) else {
        return;
    };
    let global = js_sys::global();
    let Ok(callback) = Reflect::get(&global, &JsValue::from_str("bpRendererEvent")) else {
        return;
    };
    if let Some(callback) = callback.dyn_ref::<Function>() {
        let _ = callback.call1(&JsValue::UNDEFINED, &value);
    }
}

fn flush_events(app: &mut RendererApp) {
    for event in app.drain_events() {
        emit(&event);
    }
}

enum CanvasTarget {
    Html(web_sys::HtmlCanvasElement),
    Offscreen(web_sys::OffscreenCanvas),
}

impl CanvasTarget {
    fn render_size(&self) -> (u32, u32) {
        match self {
            Self::Html(canvas) => (
                canvas.client_width().max(1) as u32,
                canvas.client_height().max(1) as u32,
            ),
            Self::Offscreen(canvas) => (canvas.width().max(1), canvas.height().max(1)),
        }
    }

    fn set_size(&self, width: u32, height: u32) {
        match self {
            Self::Html(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
            Self::Offscreen(canvas) => {
                canvas.set_width(width);
                canvas.set_height(height);
            }
        }
    }
}

struct Runtime {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    canvas: CanvasTarget,
    context: gpu2d::Context,
    gpu2d_renderer: gpu2d::Renderer,
    app: RendererApp,
    input: RawInput,
    last_frame_ms: Option<f64>,
}

impl Runtime {
    fn resize(&mut self) -> bool {
        let (width, height) = self.canvas.render_size();
        if self.config.width == width && self.config.height == height {
            return false;
        }
        self.canvas.set_size(width, height);
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        true
    }

    fn resize_to(&mut self, width: u32, height: u32) {
        self.canvas.set_size(width.max(1), height.max(1));
        self.resize();
    }

    fn frame(&mut self, timestamp_ms: f64) -> &'static str {
        self.context.take_repaint_request();
        self.resize();
        let stable_dt = self
            .last_frame_ms
            .replace(timestamp_ms)
            .map(|previous| ((timestamp_ms - previous) / 1000.0) as f32)
            .unwrap_or(1.0 / 60.0)
            .clamp(1.0 / 240.0, 0.1);
        let (input, response) = self
            .input
            .frame(self.config.width, self.config.height, stable_dt);
        self.context.reset_cursor_icon();
        let mut ui = gpu2d::Ui::new(
            self.context.clone(),
            gpu2d::vec2(self.config.width as f32, self.config.height as f32),
            input,
            response,
        );
        self.app.ui(&mut ui);
        let commands = ui.take_commands();
        flush_events(&mut self.app);

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => frame,
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("wgpu surface frame failed validation");
                self.context.request_repaint();
                return self.context.cursor_icon().css();
            }
        };
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        self.gpu2d_renderer.render(
            &self.device,
            &self.queue,
            &view,
            &self.context,
            commands,
            self.config.width,
            self.config.height,
        );
        self.queue.present(frame);
        self.context.cursor_icon().css()
    }
}

#[wasm_bindgen]
pub struct RendererHandle {
    runtime: Rc<RefCell<Option<Runtime>>>,
    panicked: Rc<Cell<bool>>,
}

impl Default for RendererHandle {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl RendererHandle {
    #[wasm_bindgen(constructor)]
    pub fn new() -> Self {
        console_error_panic_hook::set_once();
        Self {
            runtime: Rc::new(RefCell::new(None)),
            panicked: Rc::new(Cell::new(false)),
        }
    }

    pub async fn start(
        &self,
        canvas: web_sys::HtmlCanvasElement,
        asset_root: String,
    ) -> Result<(), JsValue> {
        let mut runtime = create_runtime(CanvasTarget::Html(canvas), &asset_root).await?;
        runtime.app.ready();
        flush_events(&mut runtime.app);
        *self.runtime.borrow_mut() = Some(runtime);
        Ok(())
    }

    pub async fn start_offscreen(
        &self,
        canvas: web_sys::OffscreenCanvas,
        asset_root: String,
        width: u32,
        height: u32,
    ) -> Result<(), JsValue> {
        canvas.set_width(width.max(1));
        canvas.set_height(height.max(1));
        let mut runtime = create_runtime(CanvasTarget::Offscreen(canvas), &asset_root).await?;
        runtime.app.ready();
        flush_events(&mut runtime.app);
        *self.runtime.borrow_mut() = Some(runtime);
        Ok(())
    }

    pub fn set_scene(&self, scene: JsValue) -> Result<(), JsValue> {
        let scene: ScenePayload = serde_wasm_bindgen::from_value(scene)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.apply_scene(scene);
        flush_events(&mut runtime.app);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn warm_up(&self) {
        badpiggies_editor_core::data::prepare_renderer_assets();
    }

    pub fn set_view(&self, view: JsValue) -> Result<(), JsValue> {
        let view: ViewPayload = serde_wasm_bindgen::from_value(view)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.apply_view(view);
        flush_events(&mut runtime.app);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn set_contraption_preview(&self, preview: JsValue) -> Result<(), JsValue> {
        let preview: ContraptionPreviewPayload = serde_wasm_bindgen::from_value(preview)
            .map_err(|error| JsValue::from_str(&error.to_string()))?;
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.set_contraption_preview(preview);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn resize(&self, width: u32, height: u32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.resize_to(width, height);
            runtime.context.request_repaint();
        }
    }

    pub fn font_backend(&self) -> String {
        self.runtime
            .borrow()
            .as_ref()
            .map(|runtime| runtime.context.font_backend())
            .unwrap_or("uninitialized")
            .to_string()
    }

    pub fn command(&self, command: &str) -> Result<(), JsValue> {
        let mut runtime = self.runtime.borrow_mut();
        let runtime = runtime
            .as_mut()
            .ok_or_else(|| JsValue::from_str("renderer is not running"))?;
        runtime.app.command(command).map_err(JsValue::from_str)?;
        flush_events(&mut runtime.app);
        runtime.context.request_repaint();
        Ok(())
    }

    pub fn has_panicked(&self) -> bool {
        self.panicked.get()
    }

    pub fn destroy(&self) {
        *self.runtime.borrow_mut() = None;
    }

    pub fn frame(&self, timestamp_ms: f64) -> String {
        self.runtime
            .borrow_mut()
            .as_mut()
            .map(|runtime| runtime.frame(timestamp_ms).to_string())
            .unwrap_or_else(|| "default".to_string())
    }

    pub fn needs_repaint(&self) -> bool {
        self.runtime
            .borrow()
            .as_ref()
            .is_some_and(|runtime| runtime.context.repaint_requested())
    }

    pub fn frame_stats(&self) -> String {
        self.runtime
            .borrow()
            .as_ref()
            .and_then(|runtime| serde_json::to_string(&runtime.gpu2d_renderer.last_stats()).ok())
            .unwrap_or_else(|| "{}".to_string())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pointer_event(
        &self,
        kind: &str,
        x: f32,
        y: f32,
        button: i16,
        detail: i16,
        alt: bool,
        ctrl: bool,
        shift: bool,
        command: bool,
        source: &str,
    ) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.pointer_event(
                kind, x, y, button, detail, alt, ctrl, shift, command, source,
            );
            runtime.context.request_repaint();
        }
    }

    pub fn wheel(&self, x: f32, y: f32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.wheel(x, y);
            runtime.context.request_repaint();
        }
    }

    pub fn key(&self, key: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime.input.key(key, alt, ctrl, shift, command);
            runtime.context.request_repaint();
        }
    }

    pub fn touch_transform(&self, zoom_delta: f32, dx: f32, dy: f32, center_x: f32, center_y: f32) {
        if let Some(runtime) = self.runtime.borrow_mut().as_mut() {
            runtime
                .input
                .touch_transform(zoom_delta, dx, dy, center_x, center_y);
            runtime.context.request_repaint();
        }
    }
}

async fn create_runtime(canvas: CanvasTarget, asset_root: &str) -> Result<Runtime, JsValue> {
    preload_assets(asset_root).await?;
    let instance = wgpu::Instance::default();
    let surface = match &canvas {
        CanvasTarget::Html(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas.clone()))
            .map_err(|error| {
                JsValue::from_str(&format!("failed to create canvas surface: {error}"))
            })?,
        CanvasTarget::Offscreen(canvas) => instance
            .create_surface(wgpu::SurfaceTarget::OffscreenCanvas(canvas.clone()))
            .map_err(|error| {
                JsValue::from_str(&format!("failed to create offscreen surface: {error}"))
            })?,
    };
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        })
        .await
        .map_err(|error| JsValue::from_str(&format!("failed to request GPU adapter: {error}")))?;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("bad_piggies_editor_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        })
        .await
        .map_err(|error| JsValue::from_str(&format!("failed to request GPU device: {error}")))?;
    let (width, height) = canvas.render_size();
    canvas.set_size(width, height);
    let mut config = surface
        .get_default_config(&adapter, width, height)
        .ok_or_else(|| JsValue::from_str("canvas surface is unsupported by the GPU adapter"))?;
    config.present_mode = wgpu::PresentMode::Fifo;
    config.desired_maximum_frame_latency = 2;
    surface.configure(&device, &config);
    let context = gpu2d::Context::new(&device, &queue);
    let gpu2d_renderer = gpu2d::Renderer::new(&device, config.format, &context);
    let app = RendererApp::new(&device, &queue, config.format);
    Ok(Runtime {
        _instance: instance,
        surface,
        device,
        queue,
        config,
        canvas,
        context,
        gpu2d_renderer,
        app,
        input: RawInput::default(),
        last_frame_ms: None,
    })
}

async fn preload_assets(asset_root: &str) -> Result<(), JsValue> {
    let missing =
        badpiggies_editor_core::data::runtime_assets::missing_runtime_assets(RENDERER_ASSETS);
    if missing.is_empty() {
        return Ok(());
    }
    let global = js_sys::global();
    let fetch = Reflect::get(&global, &JsValue::from_str("fetch"))?
        .dyn_into::<Function>()
        .map_err(|_| JsValue::from_str("global fetch is unavailable"))?;
    let mut requests = Vec::with_capacity(missing.len());
    for relative in &missing {
        let url = format!("{}/{relative}", asset_root.trim_end_matches('/'));
        let promise = fetch
            .call1(&global, &JsValue::from_str(&url))?
            .dyn_into::<js_sys::Promise>()
            .map_err(|_| JsValue::from_str("fetch did not return a Promise"))?;
        requests.push((relative.clone(), url, promise));
    }

    let mut assets = Vec::with_capacity(requests.len());
    for (relative, url, promise) in requests {
        let response = JsFuture::from(promise).await?;
        let response: web_sys::Response = response
            .dyn_into()
            .map_err(|_| JsValue::from_str("invalid asset response"))?;
        if !response.ok() {
            return Err(JsValue::from_str(&format!(
                "failed to fetch {url}: HTTP {}",
                response.status()
            )));
        }
        let buffer = JsFuture::from(response.array_buffer()?).await?;
        assets.push((relative, Uint8Array::new(&buffer).to_vec()));
    }
    install_runtime_assets(assets);
    Ok(())
}
