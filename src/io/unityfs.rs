use std::io::{Cursor, Read};
#[cfg(not(target_arch = "wasm32"))]
use std::path::Path;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;

use crate::diagnostics::error::{AppError, AppResult};

const UNITYFS_SIGNATURE: &str = "UnityFS";
const COMPRESSION_MASK: u32 = 0x3f;
const BLOCKS_AND_DIR_COMBINED: u32 = 0x40;
const BLOCKS_INFO_AT_END: u32 = 0x80;

#[cfg(not(target_arch = "wasm32"))]
fn invalid_data(message: impl Into<String>) -> AppError {
    AppError::invalid_data(message)
}

#[cfg(target_arch = "wasm32")]
fn invalid_data(message: impl Into<String>) -> AppError {
    AppError::invalid_data_key1("app_error_invalid_data", message.into())
}

#[derive(Clone, Debug)]
pub struct UnityFsEntry {
    pub name: String,
    pub flags: u32,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug)]
pub struct UnityFsBundle {
    signature: String,
    version: u32,
    unity_version: String,
    revision: String,
    entries: Vec<UnityFsEntry>,
}

#[derive(Clone, Debug)]
struct UnityFsBlockInfo {
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
}

#[derive(Clone, Debug)]
struct UnityFsDirectoryEntry {
    offset: u64,
    size: u64,
    flags: u32,
    name: String,
}

impl UnityFsBundle {
    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn from_path(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        let bytes = fs::read(path)?;
        Self::from_bytes(&bytes)
    }

    pub fn from_bytes(bytes: &[u8]) -> AppResult<Self> {
        let mut reader = Cursor::new(bytes);
        let signature = read_cstring(&mut reader)?;
        if signature != UNITYFS_SIGNATURE {
            return Err(invalid_data(format!(
                "Unsupported unity bundle signature: {signature}"
            )));
        }

        let version = read_u32_be(&mut reader)?;
        let unity_version = read_cstring(&mut reader)?;
        let revision = read_cstring(&mut reader)?;

        if version < 6 {
            return Err(invalid_data(format!(
                "Unsupported UnityFS version: {version}"
            )));
        }

        let total_file_size = read_u64_be(&mut reader)? as usize;
        let compressed_blocks_info_size = read_u32_be(&mut reader)? as usize;
        let uncompressed_blocks_info_size = read_u32_be(&mut reader)? as usize;
        let header_flags = read_u32_be(&mut reader)?;

        if total_file_size > bytes.len() {
            return Err(invalid_data(format!(
                "UnityFS file truncated: header says {total_file_size} bytes, file has {}",
                bytes.len()
            )));
        }

        let header_end = reader.position() as usize;
        let aligned_header_end = if version >= 7 {
            align_up(header_end, 16)?
        } else {
            header_end
        };
        let blocks_info_at_end = (header_flags & BLOCKS_INFO_AT_END) != 0;
        let blocks_info_offset = if blocks_info_at_end {
            total_file_size
                .checked_sub(compressed_blocks_info_size)
                .ok_or_else(|| invalid_data("Invalid UnityFS blocks info size"))?
        } else {
            aligned_header_end
        };

        let blocks_info_end = blocks_info_offset
            .checked_add(compressed_blocks_info_size)
            .ok_or_else(|| invalid_data("Invalid UnityFS blocks info range"))?;
        if blocks_info_end > bytes.len() {
            return Err(invalid_data("UnityFS blocks info extends beyond file"));
        }

        let blocks_info_bytes = decompress_bytes(
            header_flags & COMPRESSION_MASK,
            &bytes[blocks_info_offset..blocks_info_end],
            uncompressed_blocks_info_size,
        )?;

        let (block_infos, dir_entries) = parse_blocks_info(&blocks_info_bytes)?;

        let data_start = if blocks_info_at_end {
            aligned_header_end
        } else {
            blocks_info_end
        };
        let decompressed_data = decompress_data_blocks(bytes, data_start, &block_infos)?;
        let entries = materialize_entries(&decompressed_data, &dir_entries)?;

        Ok(Self {
            signature,
            version,
            unity_version,
            revision,
            entries,
        })
    }

    #[cfg(test)]
    pub fn entry_names(&self) -> Vec<String> {
        self.entries.iter().map(|entry| entry.name.clone()).collect()
    }

    pub fn read_entry(&self, name: &str) -> AppResult<Vec<u8>> {
        self.entries
            .iter()
            .find(|entry| entry.name == name)
            .map(|entry| entry.data.clone())
            .ok_or_else(|| invalid_data(format!("Bundle entry not found: {name}")))
    }

    pub fn replace_entry(&mut self, name: &str, data: Vec<u8>) -> AppResult<()> {
        let Some(entry) = self.entries.iter_mut().find(|entry| entry.name == name) else {
            return Err(invalid_data(format!(
                "Bundle entry not found: {name}"
            )));
        };
        entry.data = data;
        Ok(())
    }

    pub fn to_bytes(&self) -> AppResult<Vec<u8>> {
        let data_region = build_data_region(&self.entries);
        let data_hash = md5::compute(&data_region);
        let directory_entries = build_directory_entries(&self.entries);

        let mut blocks_info = Vec::new();
        blocks_info.extend_from_slice(&data_hash.0);
        write_u32_be(&mut blocks_info, 1);
        write_u32_be(&mut blocks_info, data_region.len() as u32);
        write_u32_be(&mut blocks_info, data_region.len() as u32);
        write_u16_be(&mut blocks_info, 0);
        write_u32_be(&mut blocks_info, directory_entries.len() as u32);
        for entry in &directory_entries {
            write_u64_be(&mut blocks_info, entry.offset);
            write_u64_be(&mut blocks_info, entry.size);
            write_u32_be(&mut blocks_info, entry.flags);
            write_cstring(&mut blocks_info, &entry.name);
        }

        let header_size = cstring_size(&self.signature)
            + 4
            + cstring_size(&self.unity_version)
            + cstring_size(&self.revision)
            + 8
            + 4
            + 4
            + 4;
        let aligned_header_size = if self.version >= 7 {
            align_up(header_size, 16)?
        } else {
            header_size
        };
        let total_file_size = aligned_header_size + blocks_info.len() + data_region.len();

        let mut out = Vec::with_capacity(total_file_size);
        write_cstring(&mut out, &self.signature);
        write_u32_be(&mut out, self.version);
        write_cstring(&mut out, &self.unity_version);
        write_cstring(&mut out, &self.revision);
        write_u64_be(&mut out, total_file_size as u64);
        write_u32_be(&mut out, blocks_info.len() as u32);
        write_u32_be(&mut out, blocks_info.len() as u32);
        write_u32_be(&mut out, BLOCKS_AND_DIR_COMBINED);
        if self.version >= 7 {
            pad_to_alignment(&mut out, 16);
        }
        out.extend_from_slice(&blocks_info);
        out.extend_from_slice(&data_region);

        Ok(out)
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn write_to_path(&self, path: impl AsRef<Path>) -> AppResult<()> {
        fs::write(path, self.to_bytes()?)?;
        Ok(())
    }
}

fn materialize_entries(
    data_region: &[u8],
    dir_entries: &[UnityFsDirectoryEntry],
) -> AppResult<Vec<UnityFsEntry>> {
    let mut entries = Vec::with_capacity(dir_entries.len());
    for entry in dir_entries {
        let start = entry.offset as usize;
        let end = start
            .checked_add(entry.size as usize)
            .ok_or_else(|| invalid_data(format!("Bundle entry size overflow: {}", entry.name)))?;
        if end > data_region.len() {
            return Err(invalid_data(format!(
                "Bundle entry out of range: {}",
                entry.name
            )));
        }
        entries.push(UnityFsEntry {
            name: entry.name.clone(),
            flags: entry.flags,
            data: data_region[start..end].to_vec(),
        });
    }
    Ok(entries)
}

fn build_data_region(entries: &[UnityFsEntry]) -> Vec<u8> {
    let total_size: usize = entries.iter().map(|entry| entry.data.len()).sum();
    let mut data = Vec::with_capacity(total_size);
    for entry in entries {
        data.extend_from_slice(&entry.data);
    }
    data
}

fn build_directory_entries(entries: &[UnityFsEntry]) -> Vec<UnityFsDirectoryEntry> {
    let mut offset = 0u64;
    let mut out = Vec::with_capacity(entries.len());
    for entry in entries {
        let size = entry.data.len() as u64;
        out.push(UnityFsDirectoryEntry {
            offset,
            size,
            flags: entry.flags,
            name: entry.name.clone(),
        });
        offset += size;
    }
    out
}

fn parse_blocks_info(bytes: &[u8]) -> AppResult<(Vec<UnityFsBlockInfo>, Vec<UnityFsDirectoryEntry>)> {
    let mut reader = Cursor::new(bytes);
    let mut hash = [0u8; 16];
    reader.read_exact(&mut hash)?;

    let block_count = read_u32_be(&mut reader)? as usize;
    let mut blocks = Vec::with_capacity(block_count);
    for _ in 0..block_count {
        blocks.push(UnityFsBlockInfo {
            uncompressed_size: read_u32_be(&mut reader)?,
            compressed_size: read_u32_be(&mut reader)?,
            flags: read_u16_be(&mut reader)?,
        });
    }

    let dir_count = read_u32_be(&mut reader)? as usize;
    let mut dirs = Vec::with_capacity(dir_count);
    for _ in 0..dir_count {
        dirs.push(UnityFsDirectoryEntry {
            offset: read_u64_be(&mut reader)?,
            size: read_u64_be(&mut reader)?,
            flags: read_u32_be(&mut reader)?,
            name: read_cstring(&mut reader)?,
        });
    }

    Ok((blocks, dirs))
}

fn decompress_data_blocks(
    bytes: &[u8],
    data_start: usize,
    blocks: &[UnityFsBlockInfo],
) -> AppResult<Vec<u8>> {
    let total_size: usize = blocks
        .iter()
        .map(|block| block.uncompressed_size as usize)
        .sum();
    let mut out = Vec::with_capacity(total_size);
    let mut cursor = data_start;

    for block in blocks {
        let block_end = cursor
            .checked_add(block.compressed_size as usize)
            .ok_or_else(|| invalid_data("UnityFS block range overflow"))?;
        if block_end > bytes.len() {
            return Err(invalid_data("UnityFS block extends beyond file"));
        }
        let decompressed = decompress_bytes(
            u32::from(block.flags) & COMPRESSION_MASK,
            &bytes[cursor..block_end],
            block.uncompressed_size as usize,
        )?;
        out.extend_from_slice(&decompressed);
        cursor = block_end;
    }

    Ok(out)
}

fn decompress_bytes(kind: u32, input: &[u8], expected_size: usize) -> AppResult<Vec<u8>> {
    let output = match kind {
        0 => input.to_vec(),
        1 => decompress_lzma_bytes(input, expected_size)?,
        2 | 3 => lz4_flex::block::decompress(input, expected_size)
            .map_err(|error| invalid_data(format!("Failed to decompress UnityFS LZ4 data: {error}")))?,
        other => {
            return Err(invalid_data(format!(
                "Unsupported UnityFS compression type: {other}"
            )))
        }
    };

    if output.len() != expected_size {
        return Err(invalid_data(format!(
            "UnityFS decompressed size mismatch: expected {expected_size}, got {}",
            output.len()
        )));
    }

    Ok(output)
}

fn decompress_lzma_bytes(input: &[u8], expected_size: usize) -> AppResult<Vec<u8>> {
    let mut standard_out = Vec::with_capacity(expected_size);
    if lzma_rs::lzma_decompress(&mut Cursor::new(input), &mut standard_out).is_ok()
        && standard_out.len() == expected_size
    {
        return Ok(standard_out);
    }

    if input.len() < 5 {
        return Err(invalid_data(
            "UnityFS LZMA block is too short for raw decode",
        ));
    }

    let props = input[0];
    let mut pb = u32::from(props);
    if pb >= 225 {
        return Err(invalid_data(format!(
            "UnityFS raw LZMA properties byte is invalid: {props}"
        )));
    }

    let lc = pb % 9;
    pb /= 9;
    let lp = pb % 5;
    pb /= 5;
    let dict_size = u32::from_le_bytes([input[1], input[2], input[3], input[4]]).max(0x1000);
    let params = lzma_rs::decompress::raw::LzmaParams::new(
        lzma_rs::decompress::raw::LzmaProperties { lc, lp, pb },
        dict_size,
        Some(expected_size as u64),
    );
    let mut decoder = lzma_rs::decompress::raw::LzmaDecoder::new(params, Some(usize::MAX)).map_err(
        |error| invalid_data(format!("Failed to initialize raw UnityFS LZMA decoder: {error}")),
    )?;
    let mut out = Vec::with_capacity(expected_size);
    decoder
        .decompress(&mut Cursor::new(&input[5..]), &mut out)
        .map_err(|error| invalid_data(format!("Failed to decompress raw UnityFS LZMA data: {error}")))?;
    Ok(out)
}

fn read_cstring(reader: &mut Cursor<&[u8]>) -> AppResult<String> {
    let mut bytes = Vec::new();
    loop {
        let mut buf = [0u8; 1];
        reader.read_exact(&mut buf)?;
        if buf[0] == 0 {
            break;
        }
        bytes.push(buf[0]);
    }
    String::from_utf8(bytes).map_err(|error| invalid_data(format!("Invalid UTF-8 in UnityFS string: {error}")))
}

fn read_u16_be(reader: &mut Cursor<&[u8]>) -> AppResult<u16> {
    let mut buf = [0u8; 2];
    reader.read_exact(&mut buf)?;
    Ok(u16::from_be_bytes(buf))
}

fn read_u32_be(reader: &mut Cursor<&[u8]>) -> AppResult<u32> {
    let mut buf = [0u8; 4];
    reader.read_exact(&mut buf)?;
    Ok(u32::from_be_bytes(buf))
}

fn read_u64_be(reader: &mut Cursor<&[u8]>) -> AppResult<u64> {
    let mut buf = [0u8; 8];
    reader.read_exact(&mut buf)?;
    Ok(u64::from_be_bytes(buf))
}

fn write_cstring(out: &mut Vec<u8>, value: &str) {
    out.extend_from_slice(value.as_bytes());
    out.push(0);
}

fn write_u16_be(out: &mut Vec<u8>, value: u16) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u32_be(out: &mut Vec<u8>, value: u32) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn write_u64_be(out: &mut Vec<u8>, value: u64) {
    out.extend_from_slice(&value.to_be_bytes());
}

fn cstring_size(value: &str) -> usize {
    value.len() + 1
}

fn align_up(value: usize, alignment: usize) -> AppResult<usize> {
    if alignment == 0 {
        return Err(invalid_data("Alignment must be non-zero"));
    }
    let remainder = value % alignment;
    if remainder == 0 {
        return Ok(value);
    }
    value
        .checked_add(alignment - remainder)
        .ok_or_else(|| invalid_data("Aligned size overflow"))
}

fn pad_to_alignment(out: &mut Vec<u8>, alignment: usize) {
    let remainder = out.len() % alignment;
    if remainder != 0 {
        out.resize(out.len() + (alignment - remainder), 0);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::UnityFsBundle;
    use unity_asset::environment::Environment;

    fn sample_bundle_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../Assets/StreamingAssets/AssetBundles/Episode_1_Levels.unity3d")
    }

    #[test]
    fn parses_episode_1_bundle_top_level_entries() {
        let bundle = UnityFsBundle::from_path(sample_bundle_path()).expect("parse sample bundle");
        let actual: BTreeSet<String> = bundle.entry_names().into_iter().collect();

        assert_eq!(actual.len(), 3);
        assert!(actual.iter().any(|name| name.starts_with("CAB-") && !name.ends_with(".resource") && !name.ends_with(".resS")));
        assert!(actual.iter().any(|name| name.ends_with(".resource")));
        assert!(actual.iter().any(|name| name.ends_with(".resS")));
    }

    #[test]
    fn unity_asset_reads_episode_1_container_entries() {
        let mut environment = Environment::new();
        environment
            .load_file(sample_bundle_path())
            .expect("load sample bundle with unity-asset");

        let actual: BTreeSet<String> = environment
            .bundle_container_entries(sample_bundle_path())
            .expect("extract bundle container entries")
            .into_iter()
            .map(|entry| {
                PathBuf::from(entry.asset_path)
                    .file_name()
                    .expect("container file name")
                    .to_string_lossy()
                    .into_owned()
            })
            .collect();
        assert!(actual.len() >= 40, "expected many container entries, got {}", actual.len());
        assert!(actual.contains("level_05_data.bytes"));
        assert!(actual.contains("level_49_data.bytes"));
        assert!(actual.contains("comic_episode_intro_01.png"));
    }

    #[test]
    fn can_replace_entry_and_reparse_written_bundle() {
        let mut bundle = UnityFsBundle::from_path(sample_bundle_path()).expect("parse sample bundle");
        let entry_name = bundle
            .entry_names()
            .into_iter()
            .find(|name| name.starts_with("CAB-") && !name.ends_with(".resource") && !name.ends_with(".resS"))
            .expect("top-level CAB entry");
        let mut replacement = bundle
            .read_entry(&entry_name)
            .expect("read existing entry");
        replacement.extend_from_slice(b"bundle-test");
        bundle
            .replace_entry(&entry_name, replacement.clone())
            .expect("replace entry");

        let temp_path = std::env::temp_dir().join(format!(
            "badpiggies-unityfs-{}.unity3d",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        ));

        bundle.write_to_path(&temp_path).expect("write bundle");
        let reparsed = UnityFsBundle::from_path(&temp_path).expect("reparse written bundle");
        let reparsed_entry = reparsed
            .read_entry(&entry_name)
            .expect("read replaced entry");
        assert_eq!(reparsed_entry, replacement);

        let _ = std::fs::remove_file(temp_path);
    }
}