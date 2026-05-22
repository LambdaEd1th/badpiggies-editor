use std::env;
use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use tar::Archive;

const ASSET_DIR_ENV: &str = "BP_EDITOR_UNITY_ASSETS_DIR";
const PACKAGE_PATH_ENV: &str = "BP_EDITOR_UNITYPACKAGE_PATH";
const CACHE_DIR_ENV: &str = "BP_EDITOR_UNITY_ASSET_CACHE_DIR";

const DEFAULT_PACKAGE_RELATIVE_PATH: &str =
    "assets/data/Bad-Piggies-2.3.6-Unity-Windows.unitypackage";

fn main() -> Result<(), Box<dyn Error>> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed={DEFAULT_PACKAGE_RELATIVE_PATH}");
    println!("cargo:rerun-if-env-changed={ASSET_DIR_ENV}");
    println!("cargo:rerun-if-env-changed={PACKAGE_PATH_ENV}");
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

    prepare_fetched_asset_dir(manifest_dir)
}

fn prepare_fetched_asset_dir(manifest_dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let cache_root = if let Some(path) = env::var_os(CACHE_DIR_ENV) {
        resolve_env_path(manifest_dir, path)
    } else {
        default_cache_root(manifest_dir)
    };

    let package_path = if let Some(path) = env::var_os(PACKAGE_PATH_ENV) {
        resolve_env_path(manifest_dir, path)
    } else {
        default_package_path(manifest_dir)
    };
    println!("cargo:rerun-if-changed={}", package_path.display());

    ensure_unitypackage_exists(&package_path)?;
    let sha256 = package_sha256(&package_path)?;
    let cache_root = cache_root.join(format!("unitypackage_{}", &sha256[..12]));
    fs::create_dir_all(&cache_root)?;

    let extracted_root = cache_root.join("unity_assets");
    let stamp = cache_root.join("extract.ok");
    println!("cargo:rerun-if-changed={}", stamp.display());
    if !stamp.exists() || !extracted_root.exists() {
        if extracted_root.exists() {
            fs::remove_dir_all(&extracted_root)?;
        }
        fs::create_dir_all(&extracted_root)?;
        extract_guid_layout(&package_path, &extracted_root)?;
        fs::write(&stamp, format!("sha256={sha256}\n"))?;
    }

    ensure_asset_dir(&extracted_root, DEFAULT_PACKAGE_RELATIVE_PATH)?;
    extracted_root.canonicalize().map_err(Into::into)
}

fn default_cache_root(manifest_dir: &Path) -> PathBuf {
    match env::var_os("CARGO_TARGET_DIR") {
        Some(path) => PathBuf::from(path).join("unity_asset_cache"),
        None => manifest_dir.join("target/unity_asset_cache"),
    }
}

fn default_package_path(manifest_dir: &Path) -> PathBuf {
    manifest_dir.join(DEFAULT_PACKAGE_RELATIVE_PATH)
}

fn ensure_unitypackage_exists(package_path: &Path) -> Result<(), Box<dyn Error>> {
    if package_path.is_file() {
        return Ok(());
    }

    Err(format!(
        "Unity package not found at {}. Place {} in the repo, or set {} to a local .unitypackage path.",
        package_path.display(),
        DEFAULT_PACKAGE_RELATIVE_PATH,
        PACKAGE_PATH_ENV,
    )
    .into())
}

fn package_sha256(path: &Path) -> Result<String, Box<dyn Error>> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(bytes)))
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
            "Unity asset directory not found at {} (source: {}). Set {} to an extracted directory, or let build.rs prepare a cached copy from the bundled Unity package.",
            path.display(),
            source,
            ASSET_DIR_ENV,
        )
        .into());
    }

    let mut dirs = fs::read_dir(path)?;
    if dirs.next().transpose()?.is_none() {
        return Err(format!("Unity asset directory is empty: {}", path.display()).into());
    }
    Ok(())
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
