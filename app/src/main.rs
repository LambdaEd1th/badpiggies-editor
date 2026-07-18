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
    dioxus::launch(app_view::App);
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
