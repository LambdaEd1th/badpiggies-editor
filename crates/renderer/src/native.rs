use std::time::Instant;

use crate::ContraptionPreviewPayload;
use crate::engine::{RawInput, RendererApp};
use crate::gpu2d::{self, RenderViewport};
use crate::{RendererEvent, ScenePayload, ViewPayload};

#[derive(Clone, Copy, Debug, Default)]
pub struct NativeViewport {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub window_width: f32,
    pub window_height: f32,
    pub corner_radius: f32,
}

impl NativeViewport {
    fn logical_size(self) -> (u32, u32) {
        (
            self.width.round().max(1.0) as u32,
            self.height.round().max(1.0) as u32,
        )
    }

    fn physical(self, surface_width: u32, surface_height: u32) -> RenderViewport {
        let scale_x = surface_width as f32 / self.window_width.max(1.0);
        let scale_y = surface_height as f32 / self.window_height.max(1.0);
        let left = (self.x * scale_x).round().max(0.0) as u32;
        let top = (self.y * scale_y).round().max(0.0) as u32;
        let right = ((self.x + self.width) * scale_x)
            .round()
            .max(left as f32 + 1.0) as u32;
        let bottom = ((self.y + self.height) * scale_y)
            .round()
            .max(top as f32 + 1.0) as u32;
        let x = left.min(surface_width.saturating_sub(1));
        let y = top.min(surface_height.saturating_sub(1));
        RenderViewport {
            x,
            y,
            width: right.min(surface_width).saturating_sub(x).max(1),
            height: bottom.min(surface_height).saturating_sub(y).max(1),
        }
    }
}

fn preferred_surface_format(
    formats: &[wgpu::TextureFormat],
    fallback: wgpu::TextureFormat,
) -> wgpu::TextureFormat {
    formats
        .iter()
        .copied()
        .find(|format| !format.is_srgb())
        .unwrap_or(fallback)
}

pub struct NativeFrame {
    pub cursor: &'static str,
    pub events: Vec<RendererEvent>,
    pub needs_repaint: bool,
}

pub struct NativeRendererHandle {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    context: gpu2d::Context,
    gpu2d_renderer: gpu2d::Renderer,
    app: RendererApp,
    input: RawInput,
    viewport: NativeViewport,
    last_frame: Option<Instant>,
    backdrop_color: wgpu::Color,
}

impl NativeRendererHandle {
    pub fn new(
        target: impl Into<wgpu::SurfaceTarget<'static>>,
        width: u32,
        height: u32,
    ) -> Result<Self, String> {
        let instance = wgpu::Instance::default();
        let surface = instance
            .create_surface(target)
            .map_err(|error| format!("failed to create native renderer surface: {error}"))?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            apply_limit_buckets: false,
        }))
        .map_err(|error| format!("failed to request native GPU adapter: {error}"))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("bad_piggies_editor_native_device"),
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            experimental_features: wgpu::ExperimentalFeatures::disabled(),
            memory_hints: wgpu::MemoryHints::MemoryUsage,
            trace: wgpu::Trace::Off,
        }))
        .map_err(|error| format!("failed to request native GPU device: {error}"))?;
        let width = width.max(1);
        let height = height.max(1);
        let mut config = surface
            .get_default_config(&adapter, width, height)
            .ok_or_else(|| {
                "native renderer surface is unsupported by the GPU adapter".to_string()
            })?;
        config.format =
            preferred_surface_format(&surface.get_capabilities(&adapter).formats, config.format);
        config.color_space = wgpu::SurfaceColorSpace::Srgb;
        config.present_mode = wgpu::PresentMode::Fifo;
        config.desired_maximum_frame_latency = 2;
        surface.configure(&device, &config);
        log::info!(
            "Native wgpu surface configured with {:?}/{:?} at {}x{}",
            config.format,
            config.color_space,
            config.width,
            config.height
        );

        let context = gpu2d::Context::new(&device, &queue);
        let gpu2d_renderer = gpu2d::Renderer::new(&device, config.format, &context);
        let mut app = RendererApp::new(&device, &queue, config.format);
        app.ready();
        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            config,
            context,
            gpu2d_renderer,
            app,
            input: RawInput::default(),
            viewport: NativeViewport {
                width: width as f32,
                height: height as f32,
                window_width: width as f32,
                window_height: height as f32,
                ..NativeViewport::default()
            },
            last_frame: None,
            backdrop_color: wgpu::Color {
                r: 16.0 / 255.0,
                g: 19.0 / 255.0,
                b: 24.0 / 255.0,
                a: 1.0,
            },
        })
    }

    pub fn set_scene(&mut self, scene: ScenePayload) {
        self.app.apply_scene(scene);
        self.context.request_repaint();
    }

    pub fn set_view(&mut self, view: ViewPayload) {
        self.app.apply_view(view);
        self.context.request_repaint();
    }

    pub fn set_contraption_preview(&mut self, preview: ContraptionPreviewPayload) {
        self.app.set_contraption_preview(preview);
        self.context.request_repaint();
    }

    pub fn set_viewport(&mut self, viewport: NativeViewport) {
        self.viewport = viewport;
        self.context.request_repaint();
    }

    pub fn set_backdrop_color(&mut self, red: u8, green: u8, blue: u8) {
        self.backdrop_color = wgpu::Color {
            r: f64::from(red) / 255.0,
            g: f64::from(green) / 255.0,
            b: f64::from(blue) / 255.0,
            a: 1.0,
        };
        self.context.request_repaint();
    }

    pub fn resize_surface(&mut self, width: u32, height: u32) {
        let width = width.max(1);
        let height = height.max(1);
        if self.config.width == width && self.config.height == height {
            return;
        }
        self.config.width = width;
        self.config.height = height;
        self.surface.configure(&self.device, &self.config);
        self.context.request_repaint();
    }

    pub fn command(&mut self, command: &str) -> Result<(), &'static str> {
        self.app.command(command)?;
        self.context.request_repaint();
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn pointer_event(
        &mut self,
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
        self.input.pointer_event(
            kind, x, y, button, detail, alt, ctrl, shift, command, source,
        );
        self.context.request_repaint();
    }

    pub fn wheel(&mut self, x: f32, y: f32) {
        self.input.wheel(x, y);
        self.context.request_repaint();
    }

    pub fn key(&mut self, key: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        self.input.key(key, alt, ctrl, shift, command);
        self.context.request_repaint();
    }

    pub fn touch_transform(
        &mut self,
        zoom_delta: f32,
        dx: f32,
        dy: f32,
        center_x: f32,
        center_y: f32,
    ) {
        self.input
            .touch_transform(zoom_delta, dx, dy, center_x, center_y);
        self.context.request_repaint();
    }

    pub fn needs_repaint(&self) -> bool {
        self.context.repaint_requested()
    }

    pub fn font_backend(&self) -> &'static str {
        self.context.font_backend()
    }

    pub fn drain_events(&mut self) -> Vec<RendererEvent> {
        self.app.drain_events()
    }

    pub fn frame(&mut self) -> NativeFrame {
        self.context.take_repaint_request();
        let now = Instant::now();
        let stable_dt = self
            .last_frame
            .replace(now)
            .map(|previous| now.duration_since(previous).as_secs_f32())
            .unwrap_or(1.0 / 60.0)
            .clamp(1.0 / 240.0, 0.1);
        let (logical_width, logical_height) = self.viewport.logical_size();
        let (input, response) = self.input.frame(logical_width, logical_height, stable_dt);
        self.context.reset_cursor_icon();
        let mut ui = gpu2d::Ui::new(
            self.context.clone(),
            gpu2d::vec2(logical_width as f32, logical_height as f32),
            input,
            response,
        );
        self.app.ui(&mut ui);
        let commands = ui.take_commands();

        let frame = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(frame)
            | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => Some(frame),
            wgpu::CurrentSurfaceTexture::Lost | wgpu::CurrentSurfaceTexture::Outdated => {
                self.surface.configure(&self.device, &self.config);
                self.context.request_repaint();
                None
            }
            wgpu::CurrentSurfaceTexture::Timeout | wgpu::CurrentSurfaceTexture::Occluded => {
                self.context.request_repaint();
                None
            }
            wgpu::CurrentSurfaceTexture::Validation => {
                log::error!("native wgpu surface frame failed validation");
                self.context.request_repaint();
                None
            }
        };
        if let Some(frame) = frame {
            let view = frame
                .texture
                .create_view(&wgpu::TextureViewDescriptor::default());
            self.gpu2d_renderer.render_in_viewport(
                &self.device,
                &self.queue,
                &view,
                &self.context,
                commands,
                logical_width,
                logical_height,
                self.viewport
                    .physical(self.config.width, self.config.height),
                self.backdrop_color,
                self.viewport.corner_radius,
            );
            self.queue.present(frame);
        }

        NativeFrame {
            cursor: self.context.cursor_icon().css(),
            events: self.app.drain_events(),
            needs_repaint: self.context.repaint_requested(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn viewport_uses_surface_to_webview_scale() {
        let viewport = NativeViewport {
            x: 100.0,
            y: 50.0,
            width: 800.0,
            height: 500.0,
            window_width: 1_000.0,
            window_height: 600.0,
            corner_radius: 24.0,
        };

        let physical = viewport.physical(2_000, 1_200);
        assert_eq!(physical.x, 200);
        assert_eq!(physical.y, 100);
        assert_eq!(physical.width, 1_600);
        assert_eq!(physical.height, 1_000);
        assert_eq!(viewport.logical_size(), (800, 500));
    }

    #[test]
    fn viewport_clamps_to_surface_edges() {
        let viewport = NativeViewport {
            x: 900.0,
            y: 550.0,
            width: 200.0,
            height: 100.0,
            window_width: 1_000.0,
            window_height: 600.0,
            corner_radius: 24.0,
        };

        let physical = viewport.physical(1_000, 600);
        assert_eq!(physical.x, 900);
        assert_eq!(physical.y, 550);
        assert_eq!(physical.width, 100);
        assert_eq!(physical.height, 50);
    }

    #[test]
    fn surface_format_prefers_existing_non_srgb_pipeline() {
        let formats = [
            wgpu::TextureFormat::Bgra8UnormSrgb,
            wgpu::TextureFormat::Bgra8Unorm,
        ];

        assert_eq!(
            preferred_surface_format(&formats, wgpu::TextureFormat::Bgra8UnormSrgb),
            wgpu::TextureFormat::Bgra8Unorm
        );
    }
}
