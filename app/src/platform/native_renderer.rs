use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::time::Duration;

use badpiggies_editor_renderer::{
    ContraptionPreviewPayload, NativeRendererHandle, NativeViewport, RendererEvent, ScenePayload,
    ViewPayload,
};
use dioxus::desktop::tao::event::{Event, WindowEvent};
use dioxus::desktop::tao::window::{Theme, Window, WindowBuilder};
use dioxus::prelude::*;

use crate::editor_state::ThemePreference;

const DARK_WINDOW_BACKGROUND: (u8, u8, u8, u8) = (21, 24, 29, 255);
const LIGHT_WINDOW_BACKGROUND: (u8, u8, u8, u8) = (238, 241, 244, 255);
const DARK_RENDERER_BACKGROUND: (u8, u8, u8) = (24, 27, 32);
const LIGHT_RENDERER_BACKGROUND: (u8, u8, u8) = (250, 250, 250);

pub fn window_builder(theme: ThemePreference) -> WindowBuilder {
    let builder = WindowBuilder::new()
        .with_transparent(true)
        .with_theme(window_theme(theme))
        .with_background_color(initial_window_background(theme));

    #[cfg(target_os = "macos")]
    {
        use dioxus::desktop::tao::platform::macos::WindowBuilderExtMacOS;
        builder.with_titlebar_transparent(false)
    }
    #[cfg(not(target_os = "macos"))]
    builder
}

fn window_theme(theme: ThemePreference) -> Option<Theme> {
    match theme {
        ThemePreference::System => None,
        ThemePreference::Light => Some(Theme::Light),
        ThemePreference::Dark => Some(Theme::Dark),
    }
}

const fn initial_window_background(theme: ThemePreference) -> (u8, u8, u8, u8) {
    match theme {
        ThemePreference::Light => LIGHT_WINDOW_BACKGROUND,
        ThemePreference::System | ThemePreference::Dark => DARK_WINDOW_BACKGROUND,
    }
}

enum RendererState {
    Ready(Box<NativeRendererHandle>),
    Failed(String),
}

#[derive(Clone)]
pub struct NativeRendererContext {
    state: Rc<RefCell<RendererState>>,
    window: Arc<Window>,
    event_queue: Rc<RefCell<Vec<RendererEvent>>>,
    event_revision: Signal<u64>,
    cursor: Signal<String>,
    frame_scheduled: Rc<Cell<bool>>,
    theme_preference: Rc<Cell<ThemePreference>>,
}

impl NativeRendererContext {
    fn with_renderer(&self, operation: impl FnOnce(&mut NativeRendererHandle)) {
        let mut state = self.state.borrow_mut();
        match &mut *state {
            RendererState::Ready(renderer) => {
                operation(renderer);
                self.window.request_redraw();
            }
            RendererState::Failed(error) => log::error!("Native renderer is unavailable: {error}"),
        }
    }

    pub fn set_scene(&self, scene: ScenePayload) {
        self.with_renderer(|renderer| renderer.set_scene(scene));
    }

    pub fn set_view(&self, view: ViewPayload) {
        self.with_renderer(|renderer| renderer.set_view(view));
    }

    pub fn set_contraption_preview(&self, preview: ContraptionPreviewPayload) {
        self.with_renderer(|renderer| renderer.set_contraption_preview(preview));
    }

    #[allow(clippy::too_many_arguments)]
    pub fn set_viewport(
        &self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        window_width: f32,
        window_height: f32,
        corner_radius: f32,
    ) {
        self.with_renderer(|renderer| {
            renderer.set_viewport(NativeViewport {
                x,
                y,
                width,
                height,
                window_width,
                window_height,
                corner_radius,
            });
        });
    }

    pub fn command(&self, command: &str) {
        self.with_renderer(|renderer| {
            if let Err(error) = renderer.command(command) {
                log::error!("Native renderer command failed: {error}");
            }
        });
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
    ) {
        self.with_renderer(|renderer| {
            renderer.pointer_event(kind, x, y, button, detail, alt, ctrl, shift, command);
        });
    }

    pub fn wheel(&self, x: f32, y: f32) {
        self.with_renderer(|renderer| renderer.wheel(x, y));
    }

    pub fn key(&self, key: &str, alt: bool, ctrl: bool, shift: bool, command: bool) {
        self.with_renderer(|renderer| renderer.key(key, alt, ctrl, shift, command));
    }

    pub fn touch_transform(&self, zoom: f32, dx: f32, dy: f32, center_x: f32, center_y: f32) {
        self.with_renderer(|renderer| {
            renderer.touch_transform(zoom, dx, dy, center_x, center_y);
        });
    }

    pub fn take_events(&mut self) -> Vec<RendererEvent> {
        let _revision = *self.event_revision.read();
        std::mem::take(&mut *self.event_queue.borrow_mut())
    }

    pub fn cursor(&self) -> String {
        self.cursor.read().clone()
    }

    pub fn set_theme(&self, preference: ThemePreference) {
        self.theme_preference.set(preference);
        self.window.set_theme(window_theme(preference));
        let resolved = match preference {
            ThemePreference::Light => Theme::Light,
            ThemePreference::Dark => Theme::Dark,
            ThemePreference::System => self.window.theme(),
        };
        self.apply_resolved_theme(resolved);
    }

    fn apply_resolved_theme(&self, theme: Theme) {
        let dark = matches!(theme, Theme::Dark);
        let window_background = if dark {
            DARK_WINDOW_BACKGROUND
        } else {
            LIGHT_WINDOW_BACKGROUND
        };
        let renderer_background = if dark {
            DARK_RENDERER_BACKGROUND
        } else {
            LIGHT_RENDERER_BACKGROUND
        };
        self.window.set_background_color(Some(window_background));
        self.with_renderer(|renderer| {
            renderer.set_backdrop_color(
                renderer_background.0,
                renderer_background.1,
                renderer_background.2,
            );
        });
    }
}

pub fn use_native_renderer() -> NativeRendererContext {
    let desktop = dioxus::desktop::use_window();
    let window = Arc::clone(&desktop.window);
    let state = use_hook({
        let window = Arc::clone(&window);
        move || {
            let size = window.inner_size();
            let state =
                match NativeRendererHandle::new(Arc::clone(&window), size.width, size.height) {
                    Ok(renderer) => {
                        log::info!(
                            "Native wgpu renderer initialized using {}",
                            renderer.font_backend()
                        );
                        RendererState::Ready(Box::new(renderer))
                    }
                    Err(error) => {
                        log::error!("Native wgpu renderer initialization failed: {error}");
                        RendererState::Failed(error)
                    }
                };
            Rc::new(RefCell::new(state))
        }
    });
    let event_queue = use_hook(|| Rc::new(RefCell::new(Vec::<RendererEvent>::new())));
    let event_revision = use_signal(|| 0_u64);
    let cursor = use_signal(|| "default".to_string());
    let context = NativeRendererContext {
        state,
        window: Arc::clone(&window),
        event_queue,
        event_revision,
        cursor,
        frame_scheduled: Rc::new(Cell::new(false)),
        theme_preference: Rc::new(Cell::new(ThemePreference::System)),
    };

    dioxus::desktop::use_wry_event_handler({
        let mut context = context.clone();
        move |event, _| {
            let window_id = context.window.id();
            match event {
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::Resized(size),
                    ..
                } if *event_window == window_id => {
                    context.with_renderer(|renderer| {
                        renderer.resize_surface(size.width, size.height);
                    });
                }
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::ScaleFactorChanged { new_inner_size, .. },
                    ..
                } if *event_window == window_id => {
                    context.with_renderer(|renderer| {
                        renderer.resize_surface(new_inner_size.width, new_inner_size.height);
                    });
                }
                Event::WindowEvent {
                    window_id: event_window,
                    event: WindowEvent::ThemeChanged(theme),
                    ..
                } if *event_window == window_id
                    && context.theme_preference.get() == ThemePreference::System =>
                {
                    context.apply_resolved_theme(*theme);
                }
                Event::RedrawRequested(event_window) if *event_window == window_id => {
                    let frame = {
                        let mut state = context.state.borrow_mut();
                        match &mut *state {
                            RendererState::Ready(renderer) => Some(renderer.frame()),
                            RendererState::Failed(_) => None,
                        }
                    };
                    if let Some(frame) = frame {
                        if !frame.events.is_empty() {
                            context.event_queue.borrow_mut().extend(frame.events);
                            let next_revision = context.event_revision.peek().wrapping_add(1);
                            context.event_revision.set(next_revision);
                        }
                        if context.cursor.peek().as_str() != frame.cursor {
                            context.cursor.set(frame.cursor.to_string());
                        }
                        if frame.needs_repaint && !context.frame_scheduled.replace(true) {
                            let window = Arc::clone(&context.window);
                            let scheduled = Rc::clone(&context.frame_scheduled);
                            spawn(async move {
                                futures_timer::Delay::new(Duration::from_millis(16)).await;
                                scheduled.set(false);
                                window.request_redraw();
                            });
                        }
                    }
                }
                _ => {}
            }
        }
    });

    context
}
