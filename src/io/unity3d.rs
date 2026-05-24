use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use unity_asset::{UnityValue, load_bundle_from_memory};
use unity_asset_binary::asset::class_ids;
use unity_asset_binary::asset::header::SerializedFileHeader;
use unity_asset_binary::asset::parse_serialized_file;
use unity_asset_binary::asset::types::{
    FileIdentifier, LocalSerializedObjectIdentifier, SerializedType,
};
use unity_asset_binary::bundle::AssetBundle;
use unity_asset_binary::reader::{BinaryReader, ByteOrder};
use unity_asset_binary::typetree::{TypeTree, serialize_object_with_typetree};

use crate::diagnostics::error::{AppError, AppResult};
use crate::io::unityfs::UnityFsBundle;

#[cfg(not(target_arch = "wasm32"))]
use std::fs;

const ASSET_BUNDLE_CLASS_ID: i32 = 142;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unity3dTextAssetEntry {
    pub asset_path: String,
    pub display_name: String,
    pub asset_index: usize,
    pub path_id: i64,
    pub bundle_asset_name: String,
}

#[derive(Debug, Clone, Copy)]
struct ObjectTableFieldOffsets {
    byte_start_offset: usize,
    byte_size_offset: usize,
}

#[derive(Debug, Clone)]
struct RawTextAssetData {
    name: String,
    script: Vec<u8>,
    trailing_bytes: Vec<u8>,
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(test), allow(dead_code))]
pub fn list_text_assets(bundle_path: impl AsRef<Path>) -> AppResult<Vec<Unity3dTextAssetEntry>> {
    let bundle_path = bundle_path.as_ref();
    let bundle_bytes = fs::read(bundle_path)?;
    list_text_assets_from_bytes(&bundle_path.display().to_string(), &bundle_bytes)
}

pub fn list_text_assets_from_bytes(
    bundle_name: &str,
    bundle_bytes: &[u8],
) -> AppResult<Vec<Unity3dTextAssetEntry>> {
    let bundle = load_bundle_from_memory(bundle_bytes.to_vec())
        .map_err(|err| invalid_data(format!("Failed to parse unity3d bundle: {err}")))?;

    let mut seen = BTreeSet::new();
    let mut entries = Vec::new();
    for (asset_index, file) in bundle.assets.iter().enumerate() {
        for object in file.object_handles() {
            if object.class_id() != ASSET_BUNDLE_CLASS_ID {
                continue;
            }

            let Ok(raw_entries) = file.assetbundle_container_raw(object.info()) else {
                continue;
            };

            for (asset_path, file_id, path_id) in raw_entries {
                if path_id == 0 {
                    continue;
                }
                let Some(target_asset_index) =
                    resolve_bundle_asset_index(&bundle, asset_index, file, file_id, path_id)
                else {
                    continue;
                };
                let Some(target_object) =
                    bundle.assets[target_asset_index].find_object_handle(path_id)
                else {
                    continue;
                };
                if target_object.class_id() != class_ids::TEXT_ASSET {
                    continue;
                }

                let dedup_key = (asset_path.clone(), target_asset_index, path_id);
                if !seen.insert(dedup_key) {
                    continue;
                }

                let bundle_asset_name = bundle
                    .asset_names
                    .get(target_asset_index)
                    .cloned()
                    .ok_or_else(|| {
                        invalid_data(format!(
                            "Bundle asset index {} out of range for {}",
                            target_asset_index, bundle_name
                        ))
                    })?;

                entries.push(Unity3dTextAssetEntry {
                    display_name: display_name_from_asset_path(&asset_path),
                    asset_path,
                    asset_index: target_asset_index,
                    path_id,
                    bundle_asset_name,
                });
            }
        }
    }

    entries.sort_by_cached_key(|entry| entry.display_name.to_ascii_lowercase());
    Ok(entries)
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(test), allow(dead_code))]
pub fn read_text_asset(
    bundle_path: impl AsRef<Path>,
    entry: &Unity3dTextAssetEntry,
) -> AppResult<Vec<u8>> {
    let bundle_path = bundle_path.as_ref();
    let bundle_bytes = fs::read(bundle_path)?;
    read_text_asset_from_bytes(&bundle_path.display().to_string(), &bundle_bytes, entry)
}

pub fn read_text_asset_from_bytes(
    bundle_name: &str,
    bundle_bytes: &[u8],
    entry: &Unity3dTextAssetEntry,
) -> AppResult<Vec<u8>> {
    let bundle = load_bundle_from_memory(bundle_bytes.to_vec())
        .map_err(|err| invalid_data(format!("Failed to parse unity3d bundle: {err}")))?;
    let file = bundle.assets.get(entry.asset_index).ok_or_else(|| {
        invalid_data(format!(
            "Bundle asset index {} out of range for {}",
            entry.asset_index, bundle_name
        ))
    })?;
    let object = file
        .find_object_handle(entry.path_id)
        .ok_or_else(|| {
            invalid_data(format!(
                "TextAsset {} was not found in {}",
                entry.asset_path, bundle_name
            ))
        })?
        .read()
        .map_err(|err| invalid_data(format!("Failed to parse TextAsset object: {err}")))?;
    read_text_asset_bytes(&object)
}

#[cfg(not(target_arch = "wasm32"))]
#[cfg_attr(not(test), allow(dead_code))]
pub fn replace_text_asset(
    bundle_path: impl AsRef<Path>,
    entry: &Unity3dTextAssetEntry,
    replacement_bytes: &[u8],
) -> AppResult<()> {
    let bundle_path = bundle_path.as_ref();
    let bundle_bytes = fs::read(bundle_path)?;
    let updated_bundle_bytes =
        replace_text_asset_in_bundle_bytes(&bundle_bytes, entry, replacement_bytes)?;
    fs::write(bundle_path, updated_bundle_bytes)?;
    Ok(())
}

pub fn replace_text_asset_in_bundle_bytes(
    bundle_bytes: &[u8],
    entry: &Unity3dTextAssetEntry,
    replacement_bytes: &[u8],
) -> AppResult<Vec<u8>> {
    let mut bundle = UnityFsBundle::from_bytes(bundle_bytes)?;
    let serialized_file_bytes = bundle.read_entry(&entry.bundle_asset_name)?;
    let updated_serialized_file = replace_text_asset_in_serialized_file(
        &serialized_file_bytes,
        entry.path_id,
        replacement_bytes,
    )?;
    bundle.replace_entry(&entry.bundle_asset_name, updated_serialized_file)?;
    bundle.to_bytes()
}

fn resolve_bundle_asset_index(
    bundle: &AssetBundle,
    source_asset_index: usize,
    source_file: &unity_asset_binary::asset::SerializedFile,
    file_id: i32,
    path_id: i64,
) -> Option<usize> {
    if file_id == 0
        && bundle.assets[source_asset_index]
            .find_object_handle(path_id)
            .is_some()
    {
        return Some(source_asset_index);
    }

    if file_id > 0 {
        let external_index = usize::try_from(file_id - 1).ok()?;
        if let Some(external) = source_file.externals.get(external_index) {
            let external_name = display_name_from_asset_path(&external.path);
            for (asset_index, asset_name) in bundle.asset_names.iter().enumerate() {
                if !asset_name.eq_ignore_ascii_case(&external.path)
                    && !display_name_from_asset_path(asset_name)
                        .eq_ignore_ascii_case(&external_name)
                {
                    continue;
                }
                if bundle.assets[asset_index]
                    .find_object_handle(path_id)
                    .is_some()
                {
                    return Some(asset_index);
                }
            }
        }
    }

    let mut matching_assets = bundle
        .assets
        .iter()
        .enumerate()
        .filter(|(_, asset)| asset.find_object_handle(path_id).is_some())
        .map(|(asset_index, _)| asset_index);
    let first_match = matching_assets.next()?;
    if matching_assets.next().is_none() {
        return Some(first_match);
    }

    None
}

fn display_name_from_asset_path(asset_path: &str) -> String {
    Path::new(asset_path)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| asset_path.to_string())
}

fn replace_text_asset_in_serialized_file(
    serialized_file_bytes: &[u8],
    path_id: i64,
    replacement_bytes: &[u8],
) -> AppResult<Vec<u8>> {
    let file = parse_serialized_file(serialized_file_bytes.to_vec())
        .map_err(|err| invalid_data(format!("Failed to parse serialized asset file: {err}")))?;
    let object_info = file.find_object(path_id).ok_or_else(|| {
        invalid_data(format!(
            "Serialized asset file does not contain path_id {path_id}"
        ))
    })?;
    let mut object = file
        .find_object_handle(path_id)
        .ok_or_else(|| invalid_data(format!("Missing object handle for path_id {path_id}")))?
        .read()
        .map_err(|err| invalid_data(format!("Failed to parse TextAsset object: {err}")))?;
    if object.class_id() != class_ids::TEXT_ASSET {
        return Err(invalid_data(format!(
            "Object {} is not a TextAsset (class_id={})",
            path_id,
            object.class_id()
        )));
    }

    let new_object_bytes = if object.get("m_Script").is_some() {
        object.set(
            "m_Script".to_owned(),
            bytes_to_unity_array_value(replacement_bytes),
        );
        let type_tree = type_tree_for_object(&file, object_info).ok_or_else(|| {
            invalid_data(format!("Missing TypeTree for TextAsset path_id {path_id}"))
        })?;
        if type_tree.is_empty() {
            let raw = parse_raw_text_asset_data(object.raw_data(), object.byte_order())?;
            serialize_raw_text_asset_data(
                &raw.name,
                replacement_bytes,
                &raw.trailing_bytes,
                object.byte_order(),
            )
        } else {
            let serialized_properties = object.as_unity_class().serialized_properties();
            serialize_object_with_typetree(type_tree, &serialized_properties).map_err(|err| {
                invalid_data(format!("Failed to serialize TextAsset object: {err}"))
            })?
        }
    } else {
        let raw = parse_raw_text_asset_data(object.raw_data(), object.byte_order())?;
        serialize_raw_text_asset_data(
            &raw.name,
            replacement_bytes,
            &raw.trailing_bytes,
            object.byte_order(),
        )
    };

    rebuild_serialized_file_bytes(serialized_file_bytes, &file, path_id, &new_object_bytes)
}

fn rebuild_serialized_file_bytes(
    serialized_file_bytes: &[u8],
    file: &unity_asset_binary::asset::SerializedFile,
    replaced_path_id: i64,
    new_object_bytes: &[u8],
) -> AppResult<Vec<u8>> {
    let data_offset = usize::try_from(file.header.data_offset).map_err(|_| {
        invalid_data(format!(
            "Serialized file data offset is too large: {}",
            file.header.data_offset
        ))
    })?;
    if data_offset > serialized_file_bytes.len() {
        return Err(invalid_data(format!(
            "Serialized file data offset {} exceeds file length {}",
            data_offset,
            serialized_file_bytes.len()
        )));
    }

    let field_offsets = locate_object_table_field_offsets(serialized_file_bytes)?;
    if field_offsets.len() != file.objects.len() {
        return Err(invalid_data(format!(
            "Object table count mismatch: located {} entries but parsed {} objects",
            field_offsets.len(),
            file.objects.len()
        )));
    }

    let mut objects_by_byte_start: Vec<_> = file.objects.iter().collect();
    objects_by_byte_start.sort_by_key(|object| object.byte_start);

    let mut rebuilt = serialized_file_bytes[..data_offset].to_vec();
    let mut rewritten_objects = HashMap::with_capacity(file.objects.len());
    let mut source_cursor = data_offset;

    for object in objects_by_byte_start {
        let original_object_start = usize::try_from(object.byte_start)
            .map_err(|_| invalid_data(format!("Object {} byte_start overflow", object.path_id)))?;
        let original_object_bytes = file
            .object_bytes(object)
            .map_err(|err| invalid_data(format!("Failed to read object bytes: {err}")))?;
        let original_object_end = original_object_start
            .checked_add(original_object_bytes.len())
            .ok_or_else(|| {
                invalid_data(format!("Object {} byte range overflow", object.path_id))
            })?;

        if original_object_start < source_cursor {
            return Err(invalid_data(format!(
                "Object {} overlaps a previous object range",
                object.path_id
            )));
        }

        rebuilt.extend_from_slice(
            serialized_file_bytes
                .get(source_cursor..original_object_start)
                .ok_or_else(|| {
                    invalid_data(format!(
                        "Object {} starts outside serialized file bounds",
                        object.path_id
                    ))
                })?,
        );

        let object_bytes = if object.path_id == replaced_path_id {
            new_object_bytes.to_vec()
        } else {
            original_object_bytes.to_vec()
        };

        let byte_start = u64::try_from(rebuilt.len())
            .map_err(|_| invalid_data("Rebuilt serialized file exceeded supported size"))?;
        let byte_size = u32::try_from(object_bytes.len()).map_err(|_| {
            invalid_data(format!(
                "Object {} is too large to encode: {} bytes",
                object.path_id,
                object_bytes.len()
            ))
        })?;
        rebuilt.extend_from_slice(&object_bytes);
        rewritten_objects.insert(object.path_id, (byte_start, byte_size));
        source_cursor = original_object_end;
    }

    rebuilt.extend_from_slice(
        serialized_file_bytes
            .get(source_cursor..)
            .ok_or_else(|| invalid_data("Serialized file tail starts outside file bounds"))?,
    );

    for (object, field_offsets) in file.objects.iter().zip(field_offsets.iter()) {
        let (new_byte_start, new_byte_size) = rewritten_objects
            .get(&object.path_id)
            .copied()
            .ok_or_else(|| invalid_data(format!("Missing rebuilt object {}", object.path_id)))?;
        let relative_start = new_byte_start
            .checked_sub(file.header.data_offset)
            .ok_or_else(|| {
                invalid_data(format!(
                    "Object {} byte_start {} precedes data offset {}",
                    object.path_id, new_byte_start, file.header.data_offset
                ))
            })?;
        write_object_byte_start(
            &mut rebuilt,
            file.header.version,
            file.header.byte_order(),
            field_offsets.byte_start_offset,
            relative_start,
        )?;
        write_u32(
            &mut rebuilt,
            file.header.byte_order(),
            field_offsets.byte_size_offset,
            new_byte_size,
        )?;
    }

    let new_file_size = u64::try_from(rebuilt.len())
        .map_err(|_| invalid_data("Rebuilt serialized file exceeded supported size"))?;
    write_serialized_file_header(&mut rebuilt, &file.header, new_file_size)?;
    Ok(rebuilt)
}

fn locate_object_table_field_offsets(
    serialized_file_bytes: &[u8],
) -> AppResult<Vec<ObjectTableFieldOffsets>> {
    let mut reader = BinaryReader::new(serialized_file_bytes, ByteOrder::Big);
    let header = SerializedFileHeader::from_reader(&mut reader)
        .map_err(|err| invalid_data(format!("Failed to parse serialized file header: {err}")))?;
    reader.set_byte_order(header.byte_order());

    if header.version >= 7 {
        reader
            .read_cstring()
            .map_err(|err| invalid_data(format!("Failed to read unity version string: {err}")))?;
    }
    if header.version >= 8 {
        reader
            .read_i32()
            .map_err(|err| invalid_data(format!("Failed to read target platform: {err}")))?;
    }
    let enable_type_tree = if header.version >= 13 {
        reader
            .read_bool()
            .map_err(|err| invalid_data(format!("Failed to read TypeTree flag: {err}")))?
    } else {
        false
    };

    let type_count = reader
        .read_i32()
        .map_err(|err| invalid_data(format!("Failed to read type count: {err}")))?;
    if type_count < 0 {
        return Err(invalid_data(format!(
            "Negative serialized type count: {type_count}"
        )));
    }
    for _ in 0..type_count {
        SerializedType::from_reader(&mut reader, header.version, enable_type_tree, false)
            .map_err(|err| invalid_data(format!("Failed to skip serialized type: {err}")))?;
    }

    let big_id_enabled = if header.version >= 7 && header.version < 14 {
        reader
            .read_i32()
            .map_err(|err| invalid_data(format!("Failed to read big ID flag: {err}")))?
            != 0
    } else {
        false
    };

    let object_count = reader
        .read_i32()
        .map_err(|err| invalid_data(format!("Failed to read object count: {err}")))?;
    if object_count < 0 {
        return Err(invalid_data(format!(
            "Negative serialized object count: {object_count}"
        )));
    }

    let mut offsets = Vec::with_capacity(object_count as usize);
    for _ in 0..object_count {
        if big_id_enabled {
            reader
                .read_i64()
                .map_err(|err| invalid_data(format!("Failed to read object path ID: {err}")))?;
        } else if header.version < 14 {
            reader
                .read_i32()
                .map_err(|err| invalid_data(format!("Failed to read object path ID: {err}")))?;
        } else {
            reader
                .align()
                .map_err(|err| invalid_data(format!("Failed to align object path ID: {err}")))?;
            reader
                .read_i64()
                .map_err(|err| invalid_data(format!("Failed to read object path ID: {err}")))?;
        }

        let byte_start_offset = usize::try_from(reader.position())
            .map_err(|_| invalid_data("Object byte_start offset overflow"))?;
        if header.version >= 22 {
            reader
                .read_i64()
                .map_err(|err| invalid_data(format!("Failed to read object byte_start: {err}")))?;
        } else {
            reader
                .read_u32()
                .map_err(|err| invalid_data(format!("Failed to read object byte_start: {err}")))?;
        }
        let byte_size_offset = usize::try_from(reader.position())
            .map_err(|_| invalid_data("Object byte_size offset overflow"))?;
        reader
            .read_u32()
            .map_err(|err| invalid_data(format!("Failed to read object byte_size: {err}")))?;
        reader
            .read_i32()
            .map_err(|err| invalid_data(format!("Failed to read object type index: {err}")))?;

        if header.version < 16 {
            reader
                .read_u16()
                .map_err(|err| invalid_data(format!("Failed to read object class ID: {err}")))?;
        }
        if header.version < 11 {
            reader
                .read_u16()
                .map_err(|err| invalid_data(format!("Failed to read is_destroyed flag: {err}")))?;
        }
        if (11..17).contains(&header.version) {
            reader.read_i16().map_err(|err| {
                invalid_data(format!("Failed to read object script type index: {err}"))
            })?;
        }
        if header.version == 15 || header.version == 16 {
            reader
                .read_u8()
                .map_err(|err| invalid_data(format!("Failed to read stripped flag: {err}")))?;
        }

        offsets.push(ObjectTableFieldOffsets {
            byte_start_offset,
            byte_size_offset,
        });
    }

    if header.version >= 11 {
        let script_count = reader
            .read_i32()
            .map_err(|err| invalid_data(format!("Failed to read script count: {err}")))?;
        if script_count < 0 {
            return Err(invalid_data(format!(
                "Negative script type count: {script_count}"
            )));
        }
        for _ in 0..script_count {
            LocalSerializedObjectIdentifier::from_reader(&mut reader, header.version)
                .map_err(|err| invalid_data(format!("Failed to skip script type: {err}")))?;
        }
    }

    let external_count = reader
        .read_i32()
        .map_err(|err| invalid_data(format!("Failed to read external count: {err}")))?;
    if external_count < 0 {
        return Err(invalid_data(format!(
            "Negative external reference count: {external_count}"
        )));
    }
    for _ in 0..external_count {
        FileIdentifier::from_reader(&mut reader, header.version)
            .map_err(|err| invalid_data(format!("Failed to skip external reference: {err}")))?;
    }

    if header.version >= 20 {
        let ref_type_count = reader
            .read_i32()
            .map_err(|err| invalid_data(format!("Failed to read ref type count: {err}")))?;
        if ref_type_count < 0 {
            return Err(invalid_data(format!(
                "Negative ref type count: {ref_type_count}"
            )));
        }
        for _ in 0..ref_type_count {
            SerializedType::from_reader(&mut reader, header.version, enable_type_tree, true)
                .map_err(|err| invalid_data(format!("Failed to skip ref type: {err}")))?;
        }
    }

    if header.version >= 5 {
        reader
            .read_cstring()
            .map_err(|err| invalid_data(format!("Failed to read user information: {err}")))?;
    }

    Ok(offsets)
}

fn write_serialized_file_header(
    bytes: &mut [u8],
    header: &SerializedFileHeader,
    file_size: u64,
) -> AppResult<()> {
    let file_size_u32 = u32::try_from(file_size).map_err(|_| {
        invalid_data(format!(
            "Serialized file exceeds 32-bit header field: {file_size}"
        ))
    })?;
    write_u32(bytes, ByteOrder::Big, 4, file_size_u32)?;
    if header.version >= 22 {
        let file_size_i64 = i64::try_from(file_size)
            .map_err(|_| invalid_data(format!("Serialized file is too large: {file_size}")))?;
        write_i64(bytes, ByteOrder::Big, 24, file_size_i64)?;
    }
    Ok(())
}

fn write_object_byte_start(
    bytes: &mut [u8],
    version: u32,
    byte_order: ByteOrder,
    offset: usize,
    relative_start: u64,
) -> AppResult<()> {
    if version >= 22 {
        let relative_start_i64 = i64::try_from(relative_start).map_err(|_| {
            invalid_data(format!("Object byte_start is too large: {relative_start}"))
        })?;
        write_i64(bytes, byte_order, offset, relative_start_i64)
    } else {
        let relative_start_u32 = u32::try_from(relative_start).map_err(|_| {
            invalid_data(format!(
                "Object byte_start exceeds 32-bit range: {relative_start}"
            ))
        })?;
        write_u32(bytes, byte_order, offset, relative_start_u32)
    }
}

fn write_u32(bytes: &mut [u8], byte_order: ByteOrder, offset: usize, value: u32) -> AppResult<()> {
    let end = offset
        .checked_add(4)
        .ok_or_else(|| invalid_data("u32 write offset overflow"))?;
    let slot = bytes
        .get_mut(offset..end)
        .ok_or_else(|| invalid_data(format!("u32 write out of bounds at offset {offset}")))?;
    let raw = match byte_order {
        ByteOrder::Big => value.to_be_bytes(),
        ByteOrder::Little => value.to_le_bytes(),
    };
    slot.copy_from_slice(&raw);
    Ok(())
}

fn write_i64(bytes: &mut [u8], byte_order: ByteOrder, offset: usize, value: i64) -> AppResult<()> {
    let end = offset
        .checked_add(8)
        .ok_or_else(|| invalid_data("i64 write offset overflow"))?;
    let slot = bytes
        .get_mut(offset..end)
        .ok_or_else(|| invalid_data(format!("i64 write out of bounds at offset {offset}")))?;
    let raw = match byte_order {
        ByteOrder::Big => value.to_be_bytes(),
        ByteOrder::Little => value.to_le_bytes(),
    };
    slot.copy_from_slice(&raw);
    Ok(())
}

fn read_text_asset_bytes(object: &unity_asset_binary::object::UnityObject) -> AppResult<Vec<u8>> {
    if object.class_id() != class_ids::TEXT_ASSET {
        return Err(invalid_data(format!(
            "Object {} is not a TextAsset (class_id={})",
            object.path_id(),
            object.class_id()
        )));
    }

    if let Some(script_value) = object.get("m_Script")
        && let Ok(bytes) = unity_value_to_bytes(script_value)
    {
        return Ok(bytes);
    }

    let raw = parse_raw_text_asset_data(object.raw_data(), object.byte_order())?;
    Ok(raw.script)
}

fn parse_raw_text_asset_data(
    raw_bytes: &[u8],
    byte_order: ByteOrder,
) -> AppResult<RawTextAssetData> {
    let mut reader = BinaryReader::new(raw_bytes, byte_order);
    let name = reader
        .read_aligned_string()
        .map_err(|err| invalid_data(format!("Failed to parse TextAsset name: {err}")))?;
    let script_len = reader
        .read_i32()
        .map_err(|err| invalid_data(format!("Failed to parse TextAsset byte length: {err}")))?;
    if script_len < 0 {
        return Err(invalid_data(format!(
            "Negative TextAsset byte length: {script_len}"
        )));
    }
    let script = reader
        .read_bytes(script_len as usize)
        .map_err(|err| invalid_data(format!("Failed to read TextAsset bytes: {err}")))?;
    reader
        .align()
        .map_err(|err| invalid_data(format!("Failed to align TextAsset byte array: {err}")))?;
    let trailing_bytes = reader.remaining_slice().to_vec();

    Ok(RawTextAssetData {
        name,
        script,
        trailing_bytes,
    })
}

fn serialize_raw_text_asset_data(
    name: &str,
    script: &[u8],
    trailing_bytes: &[u8],
    byte_order: ByteOrder,
) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(name.len() + script.len() + trailing_bytes.len() + 16);
    push_i32(
        &mut bytes,
        byte_order,
        i32::try_from(name.len()).unwrap_or(i32::MAX),
    );
    bytes.extend_from_slice(name.as_bytes());
    align_vec(&mut bytes, 4);
    push_i32(
        &mut bytes,
        byte_order,
        i32::try_from(script.len()).unwrap_or(i32::MAX),
    );
    bytes.extend_from_slice(script);
    align_vec(&mut bytes, 4);
    bytes.extend_from_slice(trailing_bytes);
    bytes
}

fn unity_value_to_bytes(value: &UnityValue) -> AppResult<Vec<u8>> {
    if let Some(bytes) = value.as_bytes() {
        return Ok(bytes.to_vec());
    }

    let Some(values) = value.as_array() else {
        return Err(invalid_data("TextAsset m_Script is not a byte array"));
    };
    let mut bytes = Vec::with_capacity(values.len());
    for value in values {
        let Some(number) = value.as_i64() else {
            return Err(invalid_data(
                "TextAsset byte array contained a non-integer value",
            ));
        };
        let byte = u8::try_from(number).map_err(|_| {
            invalid_data(format!(
                "TextAsset byte array contained out-of-range value {number}"
            ))
        })?;
        bytes.push(byte);
    }
    Ok(bytes)
}

fn bytes_to_unity_array_value(bytes: &[u8]) -> UnityValue {
    UnityValue::Array(
        bytes
            .iter()
            .map(|byte| UnityValue::Integer(i64::from(*byte)))
            .collect(),
    )
}

fn push_i32(buffer: &mut Vec<u8>, byte_order: ByteOrder, value: i32) {
    let raw = match byte_order {
        ByteOrder::Big => value.to_be_bytes(),
        ByteOrder::Little => value.to_le_bytes(),
    };
    buffer.extend_from_slice(&raw);
}

fn align_vec(buffer: &mut Vec<u8>, alignment: usize) {
    let remainder = buffer.len() % alignment;
    if remainder != 0 {
        buffer.resize(buffer.len() + (alignment - remainder), 0);
    }
}

fn type_tree_for_object<'a>(
    file: &'a unity_asset_binary::asset::SerializedFile,
    object_info: &unity_asset_binary::asset::ObjectInfo,
) -> Option<&'a TypeTree> {
    if object_info.type_index >= 0 {
        return file
            .types
            .get(object_info.type_index as usize)
            .map(|serialized_type| &serialized_type.type_tree);
    }

    file.types
        .iter()
        .find(|serialized_type| serialized_type.class_id == object_info.type_id)
        .map(|serialized_type| &serialized_type.type_tree)
}

#[cfg(not(target_arch = "wasm32"))]
fn invalid_data(message: impl Into<String>) -> AppError {
    AppError::invalid_data(message.into())
}

#[cfg(target_arch = "wasm32")]
fn invalid_data(message: impl Into<String>) -> AppError {
    AppError::invalid_data_key1("app_error_invalid_data", message.into())
}

#[cfg(test)]
mod tests {
    use super::{
        Unity3dTextAssetEntry, list_text_assets, list_text_assets_from_bytes, read_text_asset,
        read_text_asset_from_bytes, replace_text_asset, replace_text_asset_in_bundle_bytes,
    };

    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use unity_asset_binary::asset::parse_serialized_file;

    use crate::io::unityfs::UnityFsBundle;

    fn sample_bundle_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../Assets/StreamingAssets/AssetBundles/Episode_1_Levels.unity3d")
    }

    fn extracted_level_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../test_levels/assetbundles/episode_1_levels.unity3d/Level_05_data.bytes")
    }

    fn level_05_entry(entries: &[Unity3dTextAssetEntry]) -> Unity3dTextAssetEntry {
        entries
            .iter()
            .find(|entry| {
                entry
                    .display_name
                    .eq_ignore_ascii_case("Level_05_data.bytes")
            })
            .cloned()
            .expect("Level_05_data.bytes entry")
    }

    fn temp_bundle_copy_path() -> PathBuf {
        let mut path = std::env::temp_dir();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        path.push(format!("badpiggies-unity3d-test-{timestamp}.unity3d"));
        path
    }

    fn remove_file_if_exists(path: &Path) {
        if path.exists() {
            let _ = fs::remove_file(path);
        }
    }

    fn serialized_file_non_object_segments(serialized_file_bytes: &[u8]) -> Vec<Vec<u8>> {
        let file = parse_serialized_file(serialized_file_bytes.to_vec())
            .expect("parse serialized file for non-object segment scan");
        let data_offset = usize::try_from(file.header.data_offset).expect("data offset fits usize");
        let mut objects_by_byte_start: Vec<_> = file.objects.iter().collect();
        objects_by_byte_start.sort_by_key(|object| object.byte_start);

        let mut segments = Vec::new();
        let mut cursor = data_offset;
        for object in objects_by_byte_start {
            let object_start =
                usize::try_from(object.byte_start).expect("object byte_start fits usize");
            let object_bytes = file.object_bytes(object).expect("read object bytes");
            let object_end = object_start + object_bytes.len();

            if object_start > cursor {
                segments.push(serialized_file_bytes[cursor..object_start].to_vec());
            }
            cursor = object_end;
        }
        if cursor < serialized_file_bytes.len() {
            segments.push(serialized_file_bytes[cursor..].to_vec());
        }
        segments
    }

    struct TempBundleCleanup(PathBuf);

    impl Drop for TempBundleCleanup {
        fn drop(&mut self) {
            remove_file_if_exists(&self.0);
        }
    }

    #[test]
    fn lists_episode_1_level_text_assets() {
        let entries = list_text_assets(sample_bundle_path()).expect("list text assets");

        assert!(
            entries.len() >= 40,
            "expected many text assets, got {}",
            entries.len()
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.display_name == "level_05_data.bytes")
        );
        assert!(
            entries
                .iter()
                .any(|entry| entry.display_name == "level_49_data.bytes")
        );
    }

    #[test]
    fn reads_level_05_text_asset_bytes() {
        let entries = list_text_assets(sample_bundle_path()).expect("list text assets");
        let entry = level_05_entry(&entries);
        let actual = read_text_asset(sample_bundle_path(), &entry).expect("read text asset");
        let expected = fs::read(extracted_level_path()).expect("read extracted level fixture");

        assert_eq!(actual, expected);
    }

    #[test]
    fn replaces_level_05_text_asset_bytes_in_bundle() {
        let temp_path = temp_bundle_copy_path();
        fs::copy(sample_bundle_path(), &temp_path).expect("copy sample bundle");
        let _cleanup = TempBundleCleanup(temp_path.clone());

        let entries = list_text_assets(&temp_path).expect("list text assets from temp bundle");
        let entry = level_05_entry(&entries);
        let replacement = b"copilot-unity3d-test-payload".to_vec();

        replace_text_asset(&temp_path, &entry, &replacement).expect("replace text asset");

        let rewritten_entries = list_text_assets(&temp_path).expect("re-list text assets");
        let rewritten_entry = level_05_entry(&rewritten_entries);
        let actual =
            read_text_asset(&temp_path, &rewritten_entry).expect("read replaced text asset");
        assert_eq!(actual, replacement);
    }

    #[test]
    fn replacing_text_asset_preserves_non_object_segments() {
        let bundle_bytes = fs::read(sample_bundle_path()).expect("read sample bundle");
        let entries = list_text_assets(sample_bundle_path()).expect("list text assets");
        let entry = level_05_entry(&entries);

        let original_bundle =
            UnityFsBundle::from_bytes(&bundle_bytes).expect("parse original bundle");
        let original_serialized_file = original_bundle
            .read_entry(&entry.bundle_asset_name)
            .expect("read original serialized file");
        let original_segments = serialized_file_non_object_segments(&original_serialized_file);

        let replacement = vec![0x5a; 137];
        let rewritten_bundle_bytes =
            replace_text_asset_in_bundle_bytes(&bundle_bytes, &entry, &replacement)
                .expect("replace text asset in bundle bytes");
        let rewritten_bundle =
            UnityFsBundle::from_bytes(&rewritten_bundle_bytes).expect("parse rewritten bundle");
        let rewritten_serialized_file = rewritten_bundle
            .read_entry(&entry.bundle_asset_name)
            .expect("read rewritten serialized file");
        let rewritten_segments = serialized_file_non_object_segments(&rewritten_serialized_file);

        assert_eq!(rewritten_segments, original_segments);
    }

    #[test]
    #[ignore = "manual smoke test for an external unity3d bundle path"]
    fn smokes_external_bundle_from_env_path() {
        let bundle_path = std::env::var("BADPIGGIES_UNITY3D_SMOKE_PATH")
            .expect("BADPIGGIES_UNITY3D_SMOKE_PATH must point to a .unity3d file");
        let bundle_path = PathBuf::from(bundle_path);

        let entries =
            list_text_assets(&bundle_path).expect("list text assets from external bundle");
        assert!(
            !entries.is_empty(),
            "expected text assets in external bundle: {}",
            bundle_path.display()
        );

        let entry = level_05_entry(&entries);
        let actual =
            read_text_asset(&bundle_path, &entry).expect("read text asset from external bundle");
        assert!(
            !actual.is_empty(),
            "expected non-empty Level_05_data.bytes in {}",
            bundle_path.display()
        );
    }

    #[test]
    #[ignore = "manual smoke test for replacing a text asset in an external unity3d bundle path"]
    fn smokes_external_bundle_replace_from_env_path() {
        let bundle_path = std::env::var("BADPIGGIES_UNITY3D_SMOKE_PATH")
            .expect("BADPIGGIES_UNITY3D_SMOKE_PATH must point to a .unity3d file");
        let bundle_bytes = fs::read(&bundle_path).expect("read external bundle bytes");

        let entries = list_text_assets_from_bytes(&bundle_path, &bundle_bytes)
            .expect("list text assets from external bundle");
        let entry = level_05_entry(&entries);
        let replacement = b"copilot-external-unity3d-smoke".to_vec();

        let rewritten_bundle_bytes =
            replace_text_asset_in_bundle_bytes(&bundle_bytes, &entry, &replacement)
                .expect("replace text asset in external bundle bytes");
        let actual = read_text_asset_from_bytes(&bundle_path, &rewritten_bundle_bytes, &entry)
            .expect("read replaced text asset from rewritten external bundle bytes");

        assert_eq!(actual, replacement);
    }
}
