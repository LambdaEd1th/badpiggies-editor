#![forbid(unsafe_code)]

//! Bad Piggies level and save editor powered by Dioxus.

mod app_actions;
mod app_view;
mod components;
mod editor_state;
mod i18n;
mod platform;

#[cfg(not(target_arch = "wasm32"))]
mod cli {
    use std::path::{Path, PathBuf};

    use badpiggies_editor_core::diagnostics::error::{AppError, AppResult};
    use badpiggies_editor_core::domain::parser;
    use badpiggies_editor_core::io::crypto::{SaveFileType, decrypt_save_file, encrypt_save_file};
    use clap::{Parser, Subcommand, ValueEnum};

    #[derive(Parser)]
    #[command(name = "badpiggies-editor", about = "Bad Piggies Editor")]
    pub struct Cli {
        #[command(subcommand)]
        pub command: Option<Command>,
    }

    #[derive(Clone, ValueEnum)]
    pub enum SaveType {
        Progress,
        Contraption,
        Achievements,
    }

    impl From<SaveType> for SaveFileType {
        fn from(value: SaveType) -> Self {
            match value {
                SaveType::Progress => Self::Progress,
                SaveType::Contraption => Self::Contraption,
                SaveType::Achievements => Self::Achievements,
            }
        }
    }

    #[derive(Subcommand)]
    pub enum Command {
        Convert {
            input: PathBuf,
            output: PathBuf,
        },
        Decrypt {
            input: PathBuf,
            #[arg(short, long)]
            output: Option<PathBuf>,
            #[arg(short = 't', long = "type")]
            save_type: Option<SaveType>,
        },
        Encrypt {
            input: PathBuf,
            output: PathBuf,
            #[arg(short = 't', long = "type")]
            save_type: Option<SaveType>,
        },
    }

    fn resolve_type(explicit: Option<SaveType>, path: &Path) -> AppResult<SaveFileType> {
        explicit
            .map(Into::into)
            .or_else(|| {
                path.file_name()
                    .and_then(|name| name.to_str())
                    .and_then(SaveFileType::detect)
            })
            .ok_or_else(|| {
                AppError::invalid_data(format!("cannot detect save type from {}", path.display()))
            })
    }

    pub fn run(command: Command) -> AppResult<()> {
        match command {
            Command::Convert { input, output } => convert(input, output),
            Command::Decrypt {
                input,
                output,
                save_type,
            } => decrypt(input, output, save_type),
            Command::Encrypt {
                input,
                output,
                save_type,
            } => encrypt(input, output, save_type),
        }
    }

    fn convert(input: PathBuf, output: PathBuf) -> AppResult<()> {
        let input_ext = extension(&input);
        let output_ext = extension(&output);
        let input_bytes = std::fs::read(&input)?;
        let level = match input_ext.as_str() {
            "bytes" => parser::parse_level(input_bytes)?,
            "yaml" | "yml" => serde_yaml::from_slice(&input_bytes)
                .map_err(|error| AppError::invalid_data(error.to_string()))?,
            "toml" => {
                let text = String::from_utf8(input_bytes)
                    .map_err(|error| AppError::invalid_data(error.to_string()))?;
                toml::from_str(&text).map_err(|error| AppError::invalid_data(error.to_string()))?
            }
            _ => {
                return Err(AppError::invalid_data(format!(
                    "unsupported input extension: {input_ext}"
                )));
            }
        };
        let output_bytes = match output_ext.as_str() {
            "bytes" => parser::serialize_level(&level),
            "yaml" | "yml" => serde_yaml::to_string(&level)
                .map_err(|error| AppError::invalid_data(error.to_string()))?
                .into_bytes(),
            "toml" => toml::to_string_pretty(&level)
                .map_err(|error| AppError::invalid_data(error.to_string()))?
                .into_bytes(),
            _ => {
                return Err(AppError::invalid_data(format!(
                    "unsupported output extension: {output_ext}"
                )));
            }
        };
        std::fs::write(&output, output_bytes)?;
        eprintln!("{} -> {}", input.display(), output.display());
        Ok(())
    }

    fn decrypt(
        input: PathBuf,
        output: Option<PathBuf>,
        save_type: Option<SaveType>,
    ) -> AppResult<()> {
        let file_type = resolve_type(save_type, &input)?;
        let xml = decrypt_save_file(&file_type, &std::fs::read(&input)?)?;
        if let Some(output) = output {
            std::fs::write(output, xml)?;
        } else {
            use std::io::Write;
            std::io::stdout().write_all(&xml)?;
        }
        Ok(())
    }

    fn encrypt(input: PathBuf, output: PathBuf, save_type: Option<SaveType>) -> AppResult<()> {
        let file_type = resolve_type(save_type, &output)?;
        let bytes = encrypt_save_file(&file_type, &std::fs::read(input)?)?;
        std::fs::write(output, bytes)?;
        Ok(())
    }

    fn extension(path: &Path) -> String {
        path.extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase()
    }

    pub fn parse() -> Cli {
        Cli::parse()
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {
    use std::cell::{Cell, RefCell};
    use std::rc::Rc;

    use dioxus::desktop::tao::event::Event;
    use dioxus::desktop::tao::window::Window;
    use dioxus::desktop::{LogicalSize, WindowEvent};

    let cli = cli::parse();
    if let Some(command) = cli.command {
        if let Err(error) = cli::run(command) {
            eprintln!("Error: {error}");
            std::process::exit(1);
        }
        return;
    }

    let logger = env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .build();
    platform::log_buffer::init(Box::new(logger), log::LevelFilter::Debug);
    log::info!(
        "Bad Piggies Editor v{} starting on native",
        env!("CARGO_PKG_VERSION")
    );
    if let Err(error) = platform::runtime_assets::preload_required_runtime_assets() {
        log::error!("Failed to load runtime assets: {error}");
        std::process::exit(1);
    }
    log::info!("Runtime assets loaded");
    let theme = platform::read_theme_preference();
    let size = platform::read_window_size_preference();
    let window_handle = Rc::new(RefCell::new(None::<std::sync::Arc<Window>>));
    let window_scale_factor = Rc::new(Cell::new(1.0_f64));
    let last_window_size = Rc::new(Cell::new((size.width, size.height)));
    let on_window_handle = window_handle.clone();
    let on_window_scale_factor = window_scale_factor.clone();
    let event_window_handle = window_handle;
    let event_window_scale_factor = window_scale_factor;
    let event_last_window_size = last_window_size;
    let window = platform::native_renderer::window_builder(theme)
        .with_inner_size(LogicalSize::new(size.width as f64, size.height as f64))
        .with_min_inner_size(LogicalSize::new(
            platform::DESKTOP_WINDOW_MIN_WIDTH as f64,
            platform::DESKTOP_WINDOW_MIN_HEIGHT as f64,
        ));
    let config = dioxus::desktop::Config::new()
        .with_window(window)
        .with_menu(None)
        .with_on_window(move |window, _| {
            on_window_scale_factor.set(window.scale_factor());
            window.set_inner_size(LogicalSize::new(size.width as f64, size.height as f64));
            #[cfg(target_os = "macos")]
            badpiggies_editor_native_window::make_opaque(&window);
            *on_window_handle.borrow_mut() = Some(window);
        })
        .with_custom_event_handler(move |event, _| {
            let window = event_window_handle.borrow().as_ref().cloned();
            if let Some(window) = window.as_ref() {
                event_window_scale_factor.set(window.scale_factor());
            }
            match event {
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::Resized(size),
                    ..
                } if window
                    .as_ref()
                    .is_none_or(|window| window.id() == *window_id) =>
                {
                    if let Some(size) = save_desktop_window_size_from_physical(
                        size.width,
                        size.height,
                        event_window_scale_factor.get(),
                    ) {
                        event_last_window_size.set(size);
                    }
                }
                Event::WindowEvent {
                    window_id,
                    event:
                        WindowEvent::ScaleFactorChanged {
                            scale_factor,
                            new_inner_size,
                        },
                    ..
                } if window
                    .as_ref()
                    .is_none_or(|window| window.id() == *window_id) =>
                {
                    event_window_scale_factor.set(*scale_factor);
                    if let Some(size) = save_desktop_window_size_from_physical(
                        new_inner_size.width,
                        new_inner_size.height,
                        *scale_factor,
                    ) {
                        event_last_window_size.set(size);
                    }
                }
                Event::WindowEvent {
                    window_id,
                    event: WindowEvent::CloseRequested | WindowEvent::Destroyed,
                    ..
                } if window
                    .as_ref()
                    .is_some_and(|window| window.id() == *window_id) =>
                {
                    let (width, height) = event_last_window_size.get();
                    save_desktop_window_size_preference(width, height);
                }
                Event::LoopDestroyed => {
                    let (width, height) = event_last_window_size.get();
                    save_desktop_window_size_preference(width, height);
                }
                _ => {}
            }
        });
    dioxus::LaunchBuilder::desktop()
        .with_cfg(config)
        .launch(app_view::App);
}

#[cfg(not(target_arch = "wasm32"))]
fn save_desktop_window_size_from_physical(
    width: u32,
    height: u32,
    scale_factor: f64,
) -> Option<(u32, u32)> {
    if width == 0 || height == 0 || !scale_factor.is_finite() || scale_factor <= 0.0 {
        return None;
    }
    let logical_size =
        dioxus::desktop::tao::dpi::PhysicalSize::new(width, height).to_logical::<f64>(scale_factor);
    let width = logical_size.width.round().max(0.0) as u32;
    let height = logical_size.height.round().max(0.0) as u32;
    save_desktop_window_size_preference(width, height);
    Some((width, height))
}

#[cfg(not(target_arch = "wasm32"))]
fn save_desktop_window_size_preference(width: u32, height: u32) {
    if let Err(error) = platform::save_window_size_preference(width, height) {
        log::warn!("Failed to save window size preference: {error}");
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    console_error_panic_hook::set_once();
    platform::log_buffer::init_wasm(log::LevelFilter::Debug);
    log::info!(
        "Bad Piggies Editor v{} starting on WebAssembly",
        env!("CARGO_PKG_VERSION")
    );
    wasm_bindgen_futures::spawn_local(async {
        match platform::runtime_assets::preload_required_runtime_assets_with_progress(
            platform::startup::update_assets,
        )
        .await
        {
            Ok(()) => {
                log::info!("Runtime assets loaded");
                platform::startup::set_stage("Starting background workers...", "", Some(100.0));
                match platform::processing::warm_up().await {
                    Ok(()) => {
                        log::info!("Processing Worker pool is ready");
                        platform::startup::set_stage("Starting editor...", "", Some(100.0));
                        dioxus::launch(app_view::App);
                    }
                    Err(error) => {
                        log::error!("Processing Worker startup failed: {error}");
                        platform::startup::fail(&format!(
                            "Processing Worker startup failed: {error}"
                        ));
                    }
                }
            }
            Err(error) => {
                log::error!("Runtime asset preload failed: {error}");
                platform::startup::fail(&error);
            }
        }
    });
}
