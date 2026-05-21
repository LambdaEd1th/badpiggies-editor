use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

const FETCH_ENV: &str = "BP_EDITOR_FETCH_UNITY_ASSETS";
const ASSET_DIR_ENV: &str = "BP_EDITOR_UNITY_ASSETS_DIR";
const PACKAGE_PATH_ENV: &str = "BP_EDITOR_UNITYPACKAGE_PATH";
const PACKAGE_URL_ENV: &str = "BP_EDITOR_UNITYPACKAGE_URL";
const PACKAGE_SHA256_ENV: &str = "BP_EDITOR_UNITYPACKAGE_SHA256";
const CACHE_DIR_ENV: &str = "BP_EDITOR_UNITY_ASSET_CACHE_DIR";

const DEFAULT_PACKAGE_URL: &str = "https://github.com/BP-Innovation/Bad-Piggies-Original/releases/download/v2.3.6/Bad-Piggies-2.3.6-Unity-Windows.unitypackage";
const DEFAULT_PACKAGE_SHA256: &str = "2dcdf27a5df5f77ffbed744663c1eeca74f8d101f93652ed976df31241c34f57";
const PACKAGE_FILENAME: &str = "Bad-Piggies-2.3.6-Unity-Windows.unitypackage";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=unity_assets");
    println!("cargo:rerun-if-env-changed={FETCH_ENV}");
    println!("cargo:rerun-if-env-changed={ASSET_DIR_ENV}");
    println!("cargo:rerun-if-env-changed={PACKAGE_PATH_ENV}");
    println!("cargo:rerun-if-env-changed={PACKAGE_URL_ENV}");
    println!("cargo:rerun-if-env-changed={PACKAGE_SHA256_ENV}");
    println!("cargo:rerun-if-env-changed={CACHE_DIR_ENV}");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR")?);
    let asset_dir = resolve_asset_dir(&manifest_dir)?;
    write_embed_module(&asset_dir)?;
    Ok(())
}

fn resolve_asset_dir(manifest_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = env::var_os(ASSET_DIR_ENV) {
        let asset_dir = resolve_env_path(manifest_dir, path);
        ensure_asset_dir(&asset_dir, ASSET_DIR_ENV)?;
        return asset_dir.canonicalize().map_err(Into::into);
    }

    if env_flag(FETCH_ENV) {
        return prepare_fetched_asset_dir(manifest_dir);
    }

    let asset_dir = manifest_dir.join("unity_assets");
    if asset_dir.is_dir() {
        ensure_asset_dir(&asset_dir, "unity_assets/")?;
        return asset_dir.canonicalize().map_err(Into::into);
    }

    prepare_fetched_asset_dir(manifest_dir)
}

fn prepare_fetched_asset_dir(manifest_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let sha256 = env::var(PACKAGE_SHA256_ENV).unwrap_or_else(|_| DEFAULT_PACKAGE_SHA256.to_owned());
    let cache_root = if let Some(path) = env::var_os(CACHE_DIR_ENV) {
        resolve_env_path(manifest_dir, path)
    } else {
        default_cache_root(manifest_dir)
    };
    let cache_root = cache_root.join(format!("unitypackage_{}", &sha256[..12]));
    fs::create_dir_all(&cache_root)?;

    let package_path = if let Some(path) = env::var_os(PACKAGE_PATH_ENV) {
        resolve_env_path(manifest_dir, path)
    } else {
        let package_url = env::var(PACKAGE_URL_ENV).unwrap_or_else(|_| DEFAULT_PACKAGE_URL.to_owned());
        let cached_package = cache_root.join(PACKAGE_FILENAME);
        ensure_downloaded_package(&package_url, &sha256, &cached_package)?;
        cached_package
    };

    verify_sha256(&package_path, &sha256)?;

    let extracted_root = cache_root.join("unity_assets");
    let stamp = cache_root.join("extract.ok");
    if !stamp.exists() || !extracted_root.exists() {
        if extracted_root.exists() {
            fs::remove_dir_all(&extracted_root)?;
        }
        fs::create_dir_all(&extracted_root)?;
        extract_guid_layout(&package_path, &extracted_root)?;
        fs::write(&stamp, format!("sha256={sha256}\n"))?;
    }

    ensure_asset_dir(&extracted_root, FETCH_ENV)?;
    extracted_root.canonicalize().map_err(Into::into)
}

fn default_cache_root(manifest_dir: &Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => PathBuf::from(path).join("unity_asset_cache"),
        None => manifest_dir.join("target/unity_asset_cache"),
    }
}

fn ensure_downloaded_package(url: &str, sha256: &str, package_path: &Path) -> Result<(), Box<dyn Error>> {
    if package_path.exists() && verify_sha256(package_path, sha256).is_ok() {
        return Ok(());
    }
    if package_path.exists() {
        fs::remove_file(package_path)?;
    }

    println!("cargo:warning=downloading Unity assets from {url}");
    let tmp_path = package_path.with_extension("download");
    if tmp_path.exists() {
        fs::remove_file(&tmp_path)?;
    }

    download_package(url, &tmp_path)?;

    verify_sha256(&tmp_path, sha256)?;
    fs::rename(&tmp_path, package_path)?;
    Ok(())
}

fn download_package(url: &str, output_path: &Path) -> Result<(), Box<dyn Error>> {
    if try_download_with_curl(url, output_path)? {
        return Ok(());
    }

    #[cfg(windows)]
    {
        let status = Command::new("powershell")
            .args([
                "-NoLogo",
                "-NoProfile",
                "-Command",
                "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; Invoke-WebRequest -Uri $args[0] -OutFile $args[1]",
                url,
                &output_path.display().to_string(),
            ])
            .status()?;
        if status.success() {
            return Ok(());
        }
    }

    Err("failed to download unitypackage: neither curl nor PowerShell succeeded".into())
}

fn try_download_with_curl(url: &str, output_path: &Path) -> Result<bool, Box<dyn Error>> {
    let output = output_path.display().to_string();
    for candidate in curl_candidates() {
        let status = match Command::new(candidate)
            .args([
                "--fail",
                "--location",
                "--silent",
                "--show-error",
                "--output",
                output.as_str(),
                url,
            ])
            .status()
        {
            Ok(status) => status,
            Err(err) if err.kind() == io::ErrorKind::NotFound => continue,
            Err(err) => return Err(err.into()),
        };
        return Ok(status.success());
    }
    Ok(false)
}

fn curl_candidates() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["curl.exe", "curl"]
    }

    #[cfg(not(windows))]
    {
        &["/usr/bin/curl", "curl"]
    }
}

fn verify_sha256(path: &Path, expected_sha256: &str) -> Result<(), Box<dyn Error>> {
    let bytes = fs::read(path)?;
    let actual = format!("{:x}", Sha256::digest(bytes));
    if actual != expected_sha256 {
        return Err(format!(
            "sha256 mismatch for {}: expected {}, got {}",
            path.display(),
            expected_sha256,
            actual,
        )
        .into());
    }
    Ok(())
}

fn extract_guid_layout(package_path: &Path, target_dir: &Path) -> Result<(), Box<dyn Error>> {
    let package = fs::File::open(package_path)?;
    let decoder = GzDecoder::new(package);
    let mut archive = Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        if !entry.header().entry_type().is_file() {
            continue;
        }
        let path = entry.path()?;
        let parts: Vec<_> = path.iter().collect();
        if parts.len() != 2 {
            continue;
        }
        let guid = parts[0].to_string_lossy();
        let leaf = parts[1].to_string_lossy();
        if !matches!(leaf.as_ref(), "asset" | "asset.meta" | "pathname") {
            continue;
        }

        let out_dir = target_dir.join(guid.as_ref());
        fs::create_dir_all(&out_dir)?;
        let out_path = out_dir.join(leaf.as_ref());
        let mut out = fs::File::create(&out_path)?;
        io::copy(&mut entry, &mut out)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let permissions = fs::Permissions::from_mode(0o644);
            fs::set_permissions(&out_path, permissions)?;
        }
    }

    Ok(())
}

fn ensure_asset_dir(path: &Path, source: &str) -> Result<(), Box<dyn Error>> {
    if !path.is_dir() {
        return Err(format!(
            "Unity asset directory not found at {} (source: {}). Set {} to an extracted directory, or let build.rs prepare a cached copy by leaving local unity_assets absent or setting {}=1.",
            path.display(),
            source,
            ASSET_DIR_ENV,
            FETCH_ENV,
        )
        .into());
    }

    let mut dirs = fs::read_dir(path)?;
    if dirs.next().transpose()?.is_none() {
        return Err(format!("Unity asset directory is empty: {}", path.display()).into());
    }
    Ok(())
}

fn env_flag(name: &str) -> bool {
    matches!(env::var(name).ok().as_deref(), Some("1" | "true" | "TRUE" | "yes" | "YES"))
}

fn resolve_env_path(manifest_dir: &Path, value: impl Into<PathBuf>) -> PathBuf {
    let path = value.into();
    if path.is_absolute() {
        path
    } else {
        manifest_dir.join(path)
    }
}

fn write_embed_module(asset_dir: &Path) -> Result<(), Box<dyn Error>> {
    let out_dir = PathBuf::from(env::var("OUT_DIR")?);
    let generated = out_dir.join("project_assets_embed.rs");
    let normalized = asset_dir.to_string_lossy().replace('\\', "/");
    let contents = format!(
        "/// Project assets exposed through rust-embed on all targets.\n#[derive(rust_embed::RustEmbed)]\n#[folder = {path:?}]\npub struct ProjectAssets;\n",
        path = normalized,
    );
    fs::write(generated, contents)?;
    Ok(())
}