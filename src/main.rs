//! Bad Piggies Level Editor — Rust/egui rewrite.
//! Supports both native desktop and WASM (GitHub Pages) targets.

mod app;
mod assets;
mod bg_data;
mod crypto;
mod icon_db;
mod level_db;
mod level_ops;
mod level_refs;
mod locale;
mod log_buffer;
mod parser;
mod renderer;
mod save_parser;
mod sprite_db;
mod terrain_gen;
mod types;

use app::EditorApp;

// ── Screen size detection ────────────────────────────
/// Returns 80% of the primary monitor's logical resolution (points, not pixels).
/// Falls back to 1600×1000 on unsupported platforms or errors.
#[cfg(not(target_arch = "wasm32"))]
fn get_screen_size_80pct() -> (f32, f32) {
    #[cfg(target_os = "macos")]
    {
        use std::process::Command;
        if let Ok(out) = Command::new("system_profiler")
            .args(["SPDisplaysDataType"])
            .output()
        {
            let text = String::from_utf8_lossy(&out.stdout);
            // Prefer "UI Looks like: W x H" (logical points), fall back to "Resolution: W x H"
            for line in text.lines() {
                let trimmed = line.trim();
                if let Some(after) = trimmed.strip_prefix("UI Looks like:") {
                    // "UI Looks like: 1920 x 1080 @ 60Hz" — logical resolution
                    let parts: Vec<&str> = after.split_whitespace().collect();
                    if parts.len() >= 3
                        && let (Ok(lw), Ok(lh)) = (parts[0].parse::<f32>(), parts[2].parse::<f32>())
                    {
                        return (lw * 0.5, lh * 0.5);
                    }
                }
            }
            for line in text.lines() {
                let trimmed = line.trim();
                if trimmed.starts_with("Resolution:") {
                    let parts: Vec<&str> = trimmed.split_whitespace().collect();
                    if parts.len() >= 4
                        && let (Ok(pw), Ok(ph)) = (parts[1].parse::<f32>(), parts[3].parse::<f32>())
                    {
                        let is_retina = trimmed.contains("Retina");
                        let scale = if is_retina { 2.0 } else { 1.0 };
                        return (pw / scale * 0.5, ph / scale * 0.5);
                    }
                }
            }
        }
    }
    (1600.0, 1000.0)
}

// ── CLI ──────────────────────────────────────────────
#[cfg(not(target_arch = "wasm32"))]
mod cli {
    use crate::crypto::{SaveFileType, decrypt_save_file, encrypt_save_file};
    use crate::locale::Language;
    use clap::{Parser, Subcommand, ValueEnum};
    use std::path::PathBuf;

    #[derive(Parser)]
    #[command(name = "badpiggies-editor", about = "Bad Piggies Level Editor")]
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

    impl SaveType {
        fn into_file_type(self) -> SaveFileType {
            match self {
                Self::Progress => SaveFileType::Progress,
                Self::Contraption => SaveFileType::Contraption,
                Self::Achievements => SaveFileType::Achievements,
            }
        }
    }

    #[derive(Subcommand)]
    pub enum Command {
        /// Convert a level file between formats (bytes / yaml / toml)
        Convert {
            /// Input file (.bytes, .yaml, .yml, .toml)
            input: PathBuf,
            /// Output file (.bytes, .yaml, .yml, .toml)
            output: PathBuf,
        },
        /// Decrypt a Bad Piggies save file to XML
        Decrypt {
            /// Input save file (Progress.dat, *.contraption, Achievements.xml)
            input: PathBuf,
            /// Output XML file (defaults to stdout if omitted)
            #[arg(short, long)]
            output: Option<PathBuf>,
            /// Save file type (auto-detected from input filename if omitted)
            #[arg(short = 't', long = "type")]
            save_type: Option<SaveType>,
        },
        /// Encrypt an XML file back to Bad Piggies save format
        Encrypt {
            /// Input XML file
            input: PathBuf,
            /// Output save file (Progress.dat, *.contraption, Achievements.xml)
            output: PathBuf,
            /// Save file type (auto-detected from output filename if omitted)
            #[arg(short = 't', long = "type")]
            save_type: Option<SaveType>,
        },
    }

    /// Resolve a SaveFileType from an explicit flag or by detecting from a filename.
    fn resolve_type(
        explicit: Option<SaveType>,
        path: &std::path::Path,
        role: &str,
    ) -> Result<SaveFileType, String> {
        if let Some(st) = explicit {
            return Ok(st.into_file_type());
        }
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        SaveFileType::detect(name).ok_or_else(|| {
            format!(
                "Cannot detect save file type from {role} filename \"{name}\". \
                 Use --type progress|contraption|achievements"
            )
        })
    }

    pub fn run(cmd: Command) -> Result<(), String> {
        match cmd {
            Command::Convert { input, output } => run_convert(input, output),
            Command::Decrypt {
                input,
                output,
                save_type,
            } => run_decrypt(input, output, save_type),
            Command::Encrypt {
                input,
                output,
                save_type,
            } => run_encrypt(input, output, save_type),
        }
    }

    fn run_convert(input: PathBuf, output: PathBuf) -> Result<(), String> {
        let t = Language::from_system().i18n();

        let ext_in = input
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let ext_out = output
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();

        let display_in = input.display().to_string();
        let display_out = output.display().to_string();

        let level = match ext_in.as_str() {
            "bytes" => {
                let data = std::fs::read(&input)
                    .map_err(|e| t.fmt_path_error("cli_read_error", &display_in, &e.to_string()))?;
                crate::parser::parse_level(data)
                    .map_err(|e| t.fmt_path_error("cli_parse_error", &display_in, &e.to_string()))?
            }
            "yaml" | "yml" => {
                let text = std::fs::read_to_string(&input)
                    .map_err(|e| t.fmt_path_error("cli_read_error", &display_in, &e.to_string()))?;
                serde_yaml::from_str(&text)
                    .map_err(|e| t.fmt_path_error("cli_parse_error", &display_in, &e.to_string()))?
            }
            "toml" => {
                let text = std::fs::read_to_string(&input)
                    .map_err(|e| t.fmt_path_error("cli_read_error", &display_in, &e.to_string()))?;
                toml::from_str(&text)
                    .map_err(|e| t.fmt_path_error("cli_parse_error", &display_in, &e.to_string()))?
            }
            _ => return Err(t.fmt1("cli_unsupported_input", &ext_in)),
        };

        let output_data: Vec<u8> = match ext_out.as_str() {
            "bytes" => crate::parser::serialize_level(&level),
            "yaml" | "yml" => serde_yaml::to_string(&level)
                .map_err(|e| t.fmt1("cli_serialize_yaml_error", &e.to_string()))?
                .into_bytes(),
            "toml" => toml::to_string_pretty(&level)
                .map_err(|e| t.fmt1("cli_serialize_toml_error", &e.to_string()))?
                .into_bytes(),
            _ => return Err(t.fmt1("cli_unsupported_output", &ext_out)),
        };

        std::fs::write(&output, &output_data)
            .map_err(|e| t.fmt_path_error("cli_write_error", &display_out, &e.to_string()))?;

        eprintln!(
            "{}",
            t.fmt_convert_ok(
                &display_in,
                &display_out,
                level.objects.len(),
                level.roots.len()
            ),
        );
        Ok(())
    }

    fn run_decrypt(
        input: PathBuf,
        output: Option<PathBuf>,
        save_type: Option<SaveType>,
    ) -> Result<(), String> {
        let file_type = resolve_type(save_type, &input, "input")?;

        let data = std::fs::read(&input)
            .map_err(|e| format!("Failed to read {}: {e}", input.display()))?;
        let xml = decrypt_save_file(&file_type, &data)?;

        if let Some(ref out) = output {
            std::fs::write(out, &xml)
                .map_err(|e| format!("Failed to write {}: {e}", out.display()))?;
            eprintln!(
                "Decrypted {} ({}) -> {} ({} bytes)",
                input.display(),
                file_type.label(),
                out.display(),
                xml.len()
            );
        } else {
            use std::io::Write;
            std::io::stdout()
                .write_all(&xml)
                .map_err(|e| format!("Failed to write to stdout: {e}"))?;
        }
        Ok(())
    }

    fn run_encrypt(
        input: PathBuf,
        output: PathBuf,
        save_type: Option<SaveType>,
    ) -> Result<(), String> {
        let file_type = resolve_type(save_type, &output, "output")?;

        let xml = std::fs::read(&input)
            .map_err(|e| format!("Failed to read {}: {e}", input.display()))?;
        let encrypted = encrypt_save_file(&file_type, &xml);

        std::fs::write(&output, &encrypted)
            .map_err(|e| format!("Failed to write {}: {e}", output.display()))?;

        eprintln!(
            "Encrypted {} -> {} ({}, {} bytes)",
            input.display(),
            output.display(),
            file_type.label(),
            encrypted.len()
        );
        Ok(())
    }
}

// ── Native entry point ───────────────────────────────
#[cfg(not(target_arch = "wasm32"))]
fn main() -> eframe::Result<()> {
    use clap::Parser;

    let cli = cli::Cli::parse();
    if let Some(cmd) = cli.command {
        if let Err(e) = cli::run(cmd) {
            eprintln!("Error: {e}");
            std::process::exit(1);
        }
        return Ok(());
    }

    let inner = env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .filter_module("egui::context", log::LevelFilter::Error)
        .build();
    log_buffer::init(Box::new(inner), log::LevelFilter::Debug);

    // Query primary monitor size via macOS Core Graphics and use 80% for the initial window.
    // Falls back to 1600×1000 on other platforms or errors.
    let (w, h) = get_screen_size_80pct();

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size([w, h])
            .with_min_inner_size([800.0, 500.0])
            .with_drag_and_drop(true),
        ..Default::default()
    };

    eframe::run_native(
        "Bad Piggies Editor",
        options,
        Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
    )
}

// ── WASM entry point ─────────────────────────────────
#[cfg(target_arch = "wasm32")]
fn main() {
    use wasm_bindgen::JsCast;

    console_error_panic_hook::set_once();
    log_buffer::init_wasm(log::LevelFilter::Debug);

    let web_options = eframe::WebOptions::default();

    wasm_bindgen_futures::spawn_local(async {
        let document = web_sys::window()
            .and_then(|w| w.document())
            .expect("failed to get document");

        let canvas = document
            .get_element_by_id("the_canvas_id")
            .expect("failed to find canvas element")
            .dyn_into::<web_sys::HtmlCanvasElement>()
            .expect("element is not a canvas");

        let start_result = eframe::WebRunner::new()
            .start(
                canvas,
                web_options,
                Box::new(|cc| Ok(Box::new(EditorApp::new(cc)))),
            )
            .await;

        // Remove loading text and show error on failure
        if let Some(loading) = document.get_element_by_id("loading_text") {
            loading.remove();
        }

        if let Err(e) = start_result
            && let Some(body) = document.body()
        {
            body.set_inner_html(&format!("<p style='color:red'>应用启动失败: {e:?}</p>"));
        }
    });
}
