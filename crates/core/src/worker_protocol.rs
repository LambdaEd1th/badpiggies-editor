use serde::{Deserialize, Serialize};

use crate::domain::parser::{parse_level, serialize_level};
use crate::domain::types::LevelData;
use crate::io::crypto::{SaveFileType, decrypt_save_file, encrypt_save_file};
use crate::io::save::parser::{SaveData, parse_save_data, serialize_save_data};
use crate::io::unity3d::{
    ExtractedUnityTextAsset, Unity3dTextAssetEntry,
    list_level_text_assets_from_serialized_file_bytes, list_text_assets_from_bytes,
    read_level_text_assets_from_serialized_file_bytes, read_text_asset_from_bytes,
    replace_text_asset_in_bundle_bytes, replace_text_asset_in_serialized_file_bytes,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LevelFormat {
    Bytes,
    Yaml,
    Toml,
}

impl LevelFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Bytes => "bytes",
            Self::Yaml => "yaml",
            Self::Toml => "toml",
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case")]
pub enum WorkerRequest {
    Ping,
    ParseLevel {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        format: LevelFormat,
    },
    SerializeLevel {
        level: LevelData,
        format: LevelFormat,
    },
    DecryptSave {
        file_type: SaveFileType,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    EncryptSave {
        file_type: SaveFileType,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    ParseSave {
        file_type: SaveFileType,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    SerializeSave {
        data: SaveData,
    },
    ListUnityTextAssets {
        bundle_name: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    ReadUnityTextAsset {
        bundle_name: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        entry: Unity3dTextAssetEntry,
    },
    ReplaceUnityTextAsset {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        entry: Unity3dTextAssetEntry,
        #[serde(with = "serde_bytes")]
        replacement: Vec<u8>,
    },
    ReplaceUnitySerializedFileTextAsset {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        entry: Unity3dTextAssetEntry,
        #[serde(with = "serde_bytes")]
        replacement: Vec<u8>,
    },
    ListUnitySerializedFileLevels {
        asset_name: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    ReadUnitySerializedFileLevels {
        asset_name: String,
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
        entries: Vec<Unity3dTextAssetEntry>,
    },
    SearchText {
        text: String,
        query: String,
        case_sensitive: bool,
    },
    Batch {
        requests: Vec<WorkerRequest>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TextMatch {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "result", rename_all = "snake_case")]
pub enum WorkerResponse {
    Pong,
    Level {
        level: LevelData,
    },
    Save {
        data: SaveData,
    },
    UnityEntries {
        entries: Vec<Unity3dTextAssetEntry>,
    },
    ExtractedUnityTextAssets {
        assets: Vec<ExtractedUnityTextAsset>,
    },
    Bytes {
        #[serde(with = "serde_bytes")]
        bytes: Vec<u8>,
    },
    TextMatches {
        matches: Vec<TextMatch>,
    },
    Batch {
        responses: Vec<WorkerResponse>,
    },
    Error {
        message: String,
    },
}

pub fn perform_worker_request(request: WorkerRequest) -> WorkerResponse {
    match perform(request) {
        Ok(response) => response,
        Err(message) => WorkerResponse::Error { message },
    }
}

fn perform(request: WorkerRequest) -> Result<WorkerResponse, String> {
    match request {
        WorkerRequest::Ping => Ok(WorkerResponse::Pong),
        WorkerRequest::ParseLevel { bytes, format } => {
            let level = match format {
                LevelFormat::Bytes => parse_level(bytes).map_err(|error| error.to_string())?,
                LevelFormat::Yaml => serde_yaml::from_slice(&bytes).map_err(|e| e.to_string())?,
                LevelFormat::Toml => {
                    let text = String::from_utf8(bytes).map_err(|e| e.to_string())?;
                    toml::from_str(&text).map_err(|e| e.to_string())?
                }
            };
            Ok(WorkerResponse::Level { level })
        }
        WorkerRequest::SerializeLevel { level, format } => {
            let bytes = match format {
                LevelFormat::Bytes => serialize_level(&level),
                LevelFormat::Yaml => serde_yaml::to_string(&level)
                    .map_err(|e| e.to_string())?
                    .into_bytes(),
                LevelFormat::Toml => toml::to_string_pretty(&level)
                    .map_err(|e| e.to_string())?
                    .into_bytes(),
            };
            Ok(WorkerResponse::Bytes { bytes })
        }
        WorkerRequest::DecryptSave { file_type, bytes } => decrypt_save_file(&file_type, &bytes)
            .map(|bytes| WorkerResponse::Bytes { bytes })
            .map_err(|error| error.to_string()),
        WorkerRequest::EncryptSave { file_type, bytes } => encrypt_save_file(&file_type, &bytes)
            .map(|bytes| WorkerResponse::Bytes { bytes })
            .map_err(|error| error.to_string()),
        WorkerRequest::ParseSave { file_type, bytes } => parse_save_data(&file_type, &bytes)
            .map(|data| WorkerResponse::Save { data })
            .map_err(|error| error.to_string()),
        WorkerRequest::SerializeSave { data } => Ok(WorkerResponse::Bytes {
            bytes: serialize_save_data(&data).into_bytes(),
        }),
        WorkerRequest::ListUnityTextAssets { bundle_name, bytes } => {
            list_text_assets_from_bytes(&bundle_name, &bytes)
                .map(|entries| WorkerResponse::UnityEntries { entries })
                .map_err(|error| error.to_string())
        }
        WorkerRequest::ReadUnityTextAsset {
            bundle_name,
            bytes,
            entry,
        } => read_text_asset_from_bytes(&bundle_name, &bytes, &entry)
            .map(|bytes| WorkerResponse::Bytes { bytes })
            .map_err(|error| error.to_string()),
        WorkerRequest::ReplaceUnityTextAsset {
            bytes,
            entry,
            replacement,
        } => replace_text_asset_in_bundle_bytes(&bytes, &entry, &replacement)
            .map(|bytes| WorkerResponse::Bytes { bytes })
            .map_err(|error| error.to_string()),
        WorkerRequest::ReplaceUnitySerializedFileTextAsset {
            bytes,
            entry,
            replacement,
        } => replace_text_asset_in_serialized_file_bytes(&bytes, &entry, &replacement)
            .map(|bytes| WorkerResponse::Bytes { bytes })
            .map_err(|error| error.to_string()),
        WorkerRequest::ListUnitySerializedFileLevels { asset_name, bytes } => {
            list_level_text_assets_from_serialized_file_bytes(&asset_name, bytes)
                .map(|entries| WorkerResponse::UnityEntries { entries })
                .map_err(|error| error.to_string())
        }
        WorkerRequest::ReadUnitySerializedFileLevels {
            asset_name,
            bytes,
            entries,
        } => read_level_text_assets_from_serialized_file_bytes(&asset_name, bytes, &entries)
            .map(|assets| WorkerResponse::ExtractedUnityTextAssets { assets })
            .map_err(|error| error.to_string()),
        WorkerRequest::SearchText {
            text,
            query,
            case_sensitive,
        } => Ok(WorkerResponse::TextMatches {
            matches: search_text_matches(&text, &query, case_sensitive),
        }),
        WorkerRequest::Batch { requests } => {
            let responses = crate::parallel::map(requests, perform_worker_request);
            Ok(WorkerResponse::Batch { responses })
        }
    }
}

pub fn search_text_matches(text: &str, query: &str, case_sensitive: bool) -> Vec<TextMatch> {
    if query.is_empty() || query.len() > text.len() {
        return Vec::new();
    }

    let (source, needle) = if case_sensitive {
        (
            std::borrow::Cow::Borrowed(text),
            std::borrow::Cow::Borrowed(query),
        )
    } else {
        (
            std::borrow::Cow::Owned(text.to_ascii_lowercase()),
            std::borrow::Cow::Owned(query.to_ascii_lowercase()),
        )
    };

    let search_range = |owned_start: usize, owned_end: usize| {
        let scan_start = floor_char_boundary(&source, owned_start);
        let overlap_end = owned_end.saturating_add(needle.len().saturating_sub(1));
        let scan_end = ceil_char_boundary(&source, overlap_end.min(source.len()));
        source[scan_start..scan_end]
            .match_indices(needle.as_ref())
            .filter_map(|(relative_start, value)| {
                let start = scan_start + relative_start;
                let end = start + value.len();
                (start >= owned_start
                    && start < owned_end
                    && text.is_char_boundary(start)
                    && text.is_char_boundary(end))
                .then_some(TextMatch { start, end })
            })
            .collect::<Vec<_>>()
    };

    #[cfg(not(target_arch = "wasm32"))]
    if source.len() >= 256 * 1024 {
        let chunk_count = rayon::current_num_threads().saturating_mul(4).max(1);
        let chunk_size = source.len().div_ceil(chunk_count).max(64 * 1024);
        let ranges = (0..source.len())
            .step_by(chunk_size)
            .map(|start| (start, start.saturating_add(chunk_size).min(source.len())))
            .collect();
        return crate::parallel::map(ranges, |(start, end)| search_range(start, end))
            .into_iter()
            .flatten()
            .collect();
    }

    search_range(0, source.len())
}

fn floor_char_boundary(text: &str, mut index: usize) -> usize {
    while index > 0 && !text.is_char_boundary(index) {
        index -= 1;
    }
    index
}

fn ceil_char_boundary(text: &str, mut index: usize) -> usize {
    while index < text.len() && !text.is_char_boundary(index) {
        index += 1;
    }
    index
}

#[cfg(test)]
mod tests {
    use super::{
        LevelFormat, TextMatch, WorkerRequest, WorkerResponse, perform_worker_request,
        search_text_matches,
    };
    use crate::domain::types::LevelData;
    use crate::io::crypto::SaveFileType;
    use crate::io::save::parser::SaveData;

    #[test]
    fn level_roundtrip_through_worker_protocol() {
        let encoded = perform_worker_request(WorkerRequest::SerializeLevel {
            level: LevelData::default(),
            format: LevelFormat::Bytes,
        });
        let WorkerResponse::Bytes { bytes } = encoded else {
            panic!("expected bytes response");
        };
        let decoded = perform_worker_request(WorkerRequest::ParseLevel {
            bytes,
            format: LevelFormat::Bytes,
        });
        assert!(matches!(decoded, WorkerResponse::Level { .. }));
    }

    #[test]
    fn save_roundtrip_through_worker_protocol() {
        let encoded = perform_worker_request(WorkerRequest::SerializeSave {
            data: SaveData::Progress(Vec::new()),
        });
        let WorkerResponse::Bytes { bytes } = encoded else {
            panic!("expected bytes response");
        };
        let decoded = perform_worker_request(WorkerRequest::ParseSave {
            file_type: SaveFileType::Progress,
            bytes,
        });
        assert!(matches!(decoded, WorkerResponse::Save { .. }));
    }

    #[test]
    fn batch_keeps_request_order_and_individual_errors() {
        let response = perform_worker_request(WorkerRequest::Batch {
            requests: vec![
                WorkerRequest::Ping,
                WorkerRequest::ParseLevel {
                    bytes: Vec::new(),
                    format: LevelFormat::Bytes,
                },
                WorkerRequest::Ping,
            ],
        });
        let WorkerResponse::Batch { responses } = response else {
            panic!("expected batch response");
        };
        assert!(matches!(responses[0], WorkerResponse::Pong));
        assert!(matches!(responses[1], WorkerResponse::Error { .. }));
        assert!(matches!(responses[2], WorkerResponse::Pong));
    }

    #[test]
    fn large_text_search_keeps_boundaries_and_order() {
        let mut text = "a".repeat(256 * 1024 - 2);
        text.push_str("needle");
        text.push_str(&"b".repeat(128 * 1024));
        text.push_str("Needle");
        let matches = search_text_matches(&text, "needle", false);
        assert_eq!(
            matches,
            vec![
                TextMatch {
                    start: 256 * 1024 - 2,
                    end: 256 * 1024 + 4,
                },
                TextMatch {
                    start: 384 * 1024 + 4,
                    end: 384 * 1024 + 10,
                },
            ]
        );
    }
}
