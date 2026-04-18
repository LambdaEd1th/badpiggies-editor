//! Parsers for decrypted Bad Piggies save file XML content.

use quick_xml::Reader;
use quick_xml::events::Event;

/// A key-value entry from Progress.dat or Settings.xml.
#[derive(Clone)]
pub struct ProgressEntry {
    pub key: String,
    pub value_type: String,
    pub value: String,
}

/// A contraption part.
#[derive(Clone)]
pub struct ContraptionPart {
    pub x: i32,
    pub y: i32,
    pub part_type: i32,
    pub custom_part_index: i32,
    pub rot: i32,
    pub flipped: bool,
}

/// An achievement entry.
#[derive(Clone)]
pub struct AchievementEntry {
    pub id: String,
    pub progress: f64,
    pub completed: bool,
    pub synced: bool,
}

/// Parsed save file content.
pub enum SaveData {
    Progress(Vec<ProgressEntry>),
    Contraption(Vec<ContraptionPart>),
    Achievements(Vec<AchievementEntry>),
}

/// Helper: get a UTF-8 attribute value by name from a quick-xml `BytesStart`.
fn attr_str(e: &quick_xml::events::BytesStart, name: &[u8]) -> Option<String> {
    e.attributes()
        .filter_map(|a| a.ok())
        .find(|a| a.key.as_ref() == name)
        .and_then(|a| String::from_utf8(a.value.to_vec()).ok())
}

/// Parse Progress.dat / Settings.xml XML.
pub fn parse_progress_xml(xml: &str) -> Result<Vec<ProgressEntry>, String> {
    let mut reader = Reader::from_str(xml);
    let mut entries = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                if tag == "data" {
                    continue;
                }
                let key = attr_str(e, b"key").unwrap_or_default();
                let value = attr_str(e, b"value").unwrap_or_default();
                entries.push(ProgressEntry {
                    key,
                    value_type: tag,
                    value,
                });
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }
    Ok(entries)
}

/// Parse ContraptionDataset XML.
pub fn parse_contraption_xml(xml: &str) -> Result<Vec<ContraptionPart>, String> {
    let mut reader = Reader::from_str(xml);
    let mut parts = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let tag = e.name();
                if tag.as_ref() != b"ContraptionDatasetUnit" {
                    continue;
                }
                let x = attr_str(e, b"x").and_then(|v| v.parse().ok()).unwrap_or(0);
                let y = attr_str(e, b"y").and_then(|v| v.parse().ok()).unwrap_or(0);
                let part_type = attr_str(e, b"partType")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let custom_part_index = attr_str(e, b"customPartIndex")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let rot = attr_str(e, b"rot")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0);
                let flipped = attr_str(e, b"flipped")
                    .map(|v| v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                parts.push(ContraptionPart {
                    x,
                    y,
                    part_type,
                    custom_part_index,
                    rot,
                    flipped,
                });
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }
    Ok(parts)
}

/// Parse Achievements.xml.
pub fn parse_achievements_xml(xml: &str) -> Result<Vec<AchievementEntry>, String> {
    let mut reader = Reader::from_str(xml);
    let mut entries = Vec::new();
    loop {
        match reader.read_event() {
            Ok(Event::Empty(ref e) | Event::Start(ref e)) => {
                let tag = e.name();
                if tag.as_ref() != b"Achievement" {
                    continue;
                }
                let id = attr_str(e, b"id").unwrap_or_default();
                let progress = attr_str(e, b"progress")
                    .and_then(|v| v.parse().ok())
                    .unwrap_or(0.0);
                let completed = attr_str(e, b"completed")
                    .map(|v| v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                let synced = attr_str(e, b"synced")
                    .map(|v| v.eq_ignore_ascii_case("true"))
                    .unwrap_or(false);
                entries.push(AchievementEntry {
                    id,
                    progress,
                    completed,
                    synced,
                });
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(format!("XML parse error: {e}")),
            _ => {}
        }
    }
    Ok(entries)
}

/// Try to detect the save file type from decrypted XML content.
pub fn detect_type_from_xml(xml: &str) -> Option<crate::crypto::SaveFileType> {
    use crate::crypto::SaveFileType;
    // Look for characteristic element names in the XML
    if xml.contains("<ContraptionDataset") || xml.contains("<ContraptionDatasetUnit") {
        Some(SaveFileType::Contraption)
    } else if xml.contains("<Achievement") {
        Some(SaveFileType::Achievements)
    } else if xml.contains("<data") {
        Some(SaveFileType::Progress)
    } else {
        None
    }
}

/// Parse decrypted XML bytes based on file type.
pub fn parse_save_data(
    file_type: &crate::crypto::SaveFileType,
    xml_bytes: &[u8],
) -> Result<SaveData, String> {
    let xml = String::from_utf8(xml_bytes.to_vec()).map_err(|e| format!("Invalid UTF-8: {e}"))?;
    // Strip BOM if present
    let xml = xml.strip_prefix('\u{feff}').unwrap_or(&xml);
    match file_type {
        crate::crypto::SaveFileType::Progress => parse_progress_xml(xml).map(SaveData::Progress),
        crate::crypto::SaveFileType::Contraption => {
            parse_contraption_xml(xml).map(SaveData::Contraption)
        }
        crate::crypto::SaveFileType::Achievements => {
            parse_achievements_xml(xml).map(SaveData::Achievements)
        }
    }
}

/// XML-escape a string for use in attribute values.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('"', "&quot;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

/// Serialize Progress entries to XML.
pub fn serialize_progress_xml(entries: &[ProgressEntry]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<data>\n");
    for entry in entries {
        xml.push_str(&format!(
            "  <{} key=\"{}\" value=\"{}\" />\n",
            xml_escape(&entry.value_type),
            xml_escape(&entry.key),
            xml_escape(&entry.value),
        ));
    }
    xml.push_str("</data>");
    xml
}

/// Serialize Contraption parts to XML.
pub fn serialize_contraption_xml(parts: &[ContraptionPart]) -> String {
    let mut xml =
        String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<ContraptionDataset>\n");
    for part in parts {
        xml.push_str(&format!(
            "  <ContraptionDatasetUnit x=\"{}\" y=\"{}\" partType=\"{}\" customPartIndex=\"{}\" rot=\"{}\" flipped=\"{}\" />\n",
            part.x,
            part.y,
            part.part_type,
            part.custom_part_index,
            part.rot,
            if part.flipped { "True" } else { "False" },
        ));
    }
    xml.push_str("</ContraptionDataset>");
    xml
}

/// Serialize Achievement entries to XML.
pub fn serialize_achievements_xml(entries: &[AchievementEntry]) -> String {
    let mut xml = String::from("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n<Achievements>\n");
    for entry in entries {
        xml.push_str(&format!(
            "  <Achievement id=\"{}\" progress=\"{}\" completed=\"{}\" synced=\"{}\" />\n",
            xml_escape(&entry.id),
            entry.progress,
            if entry.completed { "True" } else { "False" },
            if entry.synced { "True" } else { "False" },
        ));
    }
    xml.push_str("</Achievements>");
    xml
}

/// Serialize SaveData to XML string.
pub fn serialize_save_data(data: &SaveData) -> String {
    match data {
        SaveData::Progress(entries) => serialize_progress_xml(entries),
        SaveData::Contraption(parts) => serialize_contraption_xml(parts),
        SaveData::Achievements(entries) => serialize_achievements_xml(entries),
    }
}
