//! Binary level file parser and serializer.
//! Mirrors the .NET BinaryReader/BinaryWriter format used by LevelLoader.cs.

use std::io::{self, Cursor, Read};

use crate::types::*;

// ── Binary Reader ────────────────────────────────────

pub struct BinaryReader {
    cursor: Cursor<Vec<u8>>,
}

impl BinaryReader {
    pub fn new(data: Vec<u8>) -> Self {
        Self {
            cursor: Cursor::new(data),
        }
    }

    pub fn read_byte(&mut self) -> io::Result<u8> {
        let mut buf = [0u8; 1];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf[0])
    }

    pub fn read_boolean(&mut self) -> io::Result<bool> {
        Ok(self.read_byte()? != 0)
    }

    pub fn read_int16(&mut self) -> io::Result<i16> {
        let mut buf = [0u8; 2];
        self.cursor.read_exact(&mut buf)?;
        Ok(i16::from_le_bytes(buf))
    }

    pub fn read_int32(&mut self) -> io::Result<i32> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf)?;
        Ok(i32::from_le_bytes(buf))
    }

    pub fn read_uint32(&mut self) -> io::Result<u32> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf)?;
        Ok(u32::from_le_bytes(buf))
    }

    pub fn read_single(&mut self) -> io::Result<f32> {
        let mut buf = [0u8; 4];
        self.cursor.read_exact(&mut buf)?;
        Ok(f32::from_le_bytes(buf))
    }

    /// Read a .NET BinaryWriter string (7-bit encoded length prefix + UTF-8).
    pub fn read_string(&mut self) -> io::Result<String> {
        let length = self.read_7bit_encoded_int()?;
        let mut buf = vec![0u8; length as usize];
        self.cursor.read_exact(&mut buf)?;
        String::from_utf8(buf).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn read_bytes(&mut self, count: usize) -> io::Result<Vec<u8>> {
        let mut buf = vec![0u8; count];
        self.cursor.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// .NET 7-bit encoded integer (variable length).
    fn read_7bit_encoded_int(&mut self) -> io::Result<u32> {
        let mut result: u32 = 0;
        let mut shift: u32 = 0;
        loop {
            let byte = self.read_byte()?;
            result |= ((byte & 0x7f) as u32) << shift;
            shift += 7;
            if byte & 0x80 == 0 {
                break;
            }
            if shift >= 35 {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "bad 7-bit encoded int",
                ));
            }
        }
        Ok(result)
    }
}

// ── Binary Writer ────────────────────────────────────

pub struct BinaryWriter {
    buffer: Vec<u8>,
}

impl BinaryWriter {
    pub fn new() -> Self {
        Self { buffer: Vec::new() }
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.buffer
    }

    pub fn write_byte(&mut self, val: u8) {
        self.buffer.push(val);
    }

    pub fn write_boolean(&mut self, val: bool) {
        self.write_byte(if val { 1 } else { 0 });
    }

    pub fn write_int16(&mut self, val: i16) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_int32(&mut self, val: i32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_uint32(&mut self, val: u32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    pub fn write_single(&mut self, val: f32) {
        self.buffer.extend_from_slice(&val.to_le_bytes());
    }

    /// Write .NET BinaryWriter string (7-bit encoded length + UTF-8).
    pub fn write_string(&mut self, val: &str) {
        let bytes = val.as_bytes();
        self.write_7bit_encoded_int(bytes.len() as u32);
        self.buffer.extend_from_slice(bytes);
    }

    pub fn write_bytes(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    fn write_7bit_encoded_int(&mut self, mut val: u32) {
        loop {
            let mut byte = (val & 0x7f) as u8;
            val >>= 7;
            if val != 0 {
                byte |= 0x80;
            }
            self.write_byte(byte);
            if val == 0 {
                break;
            }
        }
    }
}

impl Default for BinaryWriter {
    fn default() -> Self {
        Self::new()
    }
}

// ── Level Parsing ────────────────────────────────────

/// Parse a `.bytes` level file.
pub fn parse_level(data: Vec<u8>) -> io::Result<LevelData> {
    let mut reader = BinaryReader::new(data);
    let object_count = reader.read_int32()?;

    let mut level = LevelData::default();
    for _ in 0..object_count {
        let idx = read_object(&mut reader, &mut level, None)?;
        level.roots.push(idx);
    }
    Ok(level)
}

fn read_object(
    reader: &mut BinaryReader,
    level: &mut LevelData,
    parent: Option<ObjectIndex>,
) -> io::Result<ObjectIndex> {
    let child_count = reader.read_int16()?;
    if child_count == 0 {
        read_prefab_instance(reader, level, parent)
    } else {
        read_parent_object(child_count, reader, level, parent)
    }
}

fn read_prefab_instance(
    reader: &mut BinaryReader,
    level: &mut LevelData,
    parent: Option<ObjectIndex>,
) -> io::Result<ObjectIndex> {
    let name = reader.read_string()?;
    let prefab_index = reader.read_int16()?;
    let position = read_vector3(reader)?;
    let rotation = read_vector3(reader)?;
    let scale = read_vector3(reader)?;

    let data_type_byte = reader.read_byte()?;
    let data_type = DataType::from_byte(data_type_byte);

    let terrain_data = if data_type == DataType::Terrain {
        Some(Box::new(read_terrain(reader)?))
    } else {
        None
    };

    let override_data = if data_type == DataType::PrefabOverrides {
        Some(read_prefab_overrides(reader)?)
    } else {
        None
    };

    let idx = level.objects.len();
    level.objects.push(LevelObject::Prefab(PrefabInstance {
        name,
        position,
        prefab_index,
        rotation,
        scale,
        data_type,
        terrain_data,
        override_data,
        parent,
    }));
    Ok(idx)
}

fn read_parent_object(
    child_count: i16,
    reader: &mut BinaryReader,
    level: &mut LevelData,
    parent_idx: Option<ObjectIndex>,
) -> io::Result<ObjectIndex> {
    let name = reader.read_string()?;
    let position = read_vector3(reader)?;

    // Reserve slot so children can reference this parent
    let idx = level.objects.len();
    level.objects.push(LevelObject::Parent(ParentObject {
        name,
        position,
        children: Vec::new(),
        parent: parent_idx,
    }));

    let mut children = Vec::new();
    for _ in 0..child_count {
        let child_idx = read_object(reader, level, Some(idx))?;
        children.push(child_idx);
    }

    // Patch children into the parent
    if let LevelObject::Parent(ref mut p) = level.objects[idx] {
        p.children = children;
    }

    Ok(idx)
}

fn read_vector2(reader: &mut BinaryReader) -> io::Result<Vec2> {
    Ok(Vec2 {
        x: reader.read_single()?,
        y: reader.read_single()?,
    })
}

fn read_vector3(reader: &mut BinaryReader) -> io::Result<Vec3> {
    Ok(Vec3 {
        x: reader.read_single()?,
        y: reader.read_single()?,
        z: reader.read_single()?,
    })
}

fn read_color(reader: &mut BinaryReader) -> io::Result<Color> {
    let val = reader.read_uint32()?;
    Ok(Color::from_packed(val))
}

fn read_terrain(reader: &mut BinaryReader) -> io::Result<TerrainData> {
    let fill_texture_tile_offset_x = reader.read_single()?;
    let fill_texture_tile_offset_y = reader.read_single()?;

    let fill_mesh = read_mesh(reader)?;
    let fill_color = read_color(reader)?;
    let fill_texture_index = reader.read_int32()?;

    let curve_mesh = read_mesh(reader)?;

    let curve_texture_count = reader.read_int32()?;
    let mut curve_textures = Vec::with_capacity(curve_texture_count as usize);
    for _ in 0..curve_texture_count {
        let texture_index = reader.read_int32()?;
        let size = read_vector2(reader)?;
        let fixed_angle = reader.read_boolean()?;
        let fade_threshold = reader.read_single()?;
        curve_textures.push(CurveTexture {
            texture_index,
            size,
            fixed_angle,
            fade_threshold,
        });
    }

    let control_texture_count = reader.read_int32()?;
    let control_texture_data = if control_texture_count > 0 {
        let data_length = reader.read_int32()? as usize;
        Some(reader.read_bytes(data_length)?)
    } else {
        None
    };

    let has_collider = reader.read_boolean()?;

    Ok(TerrainData {
        fill_texture_tile_offset_x,
        fill_texture_tile_offset_y,
        fill_mesh,
        fill_color,
        fill_texture_index,
        curve_mesh,
        curve_textures,
        control_texture_count,
        control_texture_data,
        has_collider,
    })
}

fn read_mesh(reader: &mut BinaryReader) -> io::Result<TerrainMesh> {
    let vertex_count = reader.read_int32()? as usize;
    let mut vertices = Vec::with_capacity(vertex_count);
    for _ in 0..vertex_count {
        vertices.push(read_vector2(reader)?);
    }

    let index_count = reader.read_int32()? as usize;
    let mut indices = Vec::with_capacity(index_count);
    for _ in 0..index_count {
        indices.push(reader.read_int16()?);
    }

    Ok(TerrainMesh { vertices, indices })
}

fn read_prefab_overrides(reader: &mut BinaryReader) -> io::Result<PrefabOverrideData> {
    let size = reader.read_int32()? as usize;
    let raw_bytes = reader.read_bytes(size)?;
    let raw_text = String::from_utf8(raw_bytes.clone())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    Ok(PrefabOverrideData {
        raw_text,
        raw_bytes,
    })
}

// ── Level Serialization ──────────────────────────────

/// Serialize level data back to `.bytes` format.
pub fn serialize_level(level: &LevelData) -> Vec<u8> {
    let mut writer = BinaryWriter::new();
    writer.write_int32(level.roots.len() as i32);
    for &root_idx in &level.roots {
        write_object(&mut writer, level, root_idx);
    }
    writer.into_bytes()
}

fn write_object(writer: &mut BinaryWriter, level: &LevelData, idx: ObjectIndex) {
    match &level.objects[idx] {
        LevelObject::Prefab(_) => {
            writer.write_int16(0);
            write_prefab_instance(writer, level, idx);
        }
        LevelObject::Parent(p) => {
            writer.write_int16(p.children.len() as i16);
            write_parent_object(writer, level, idx);
        }
    }
}

fn write_prefab_instance(writer: &mut BinaryWriter, level: &LevelData, idx: ObjectIndex) {
    let obj = level.objects[idx].as_prefab().unwrap();

    writer.write_string(&obj.name);
    writer.write_int16(obj.prefab_index);
    write_vector3(writer, &obj.position);
    write_vector3(writer, &obj.rotation);
    write_vector3(writer, &obj.scale);

    writer.write_byte(obj.data_type as u8);

    if obj.data_type == DataType::Terrain {
        if let Some(ref td) = obj.terrain_data {
            write_terrain(writer, td);
        }
    } else if obj.data_type == DataType::PrefabOverrides
        && let Some(ref od) = obj.override_data
    {
        write_prefab_overrides(writer, od);
    }
}

fn write_parent_object(writer: &mut BinaryWriter, level: &LevelData, idx: ObjectIndex) {
    let obj = level.objects[idx].as_parent().unwrap();

    writer.write_string(&obj.name);
    write_vector3(writer, &obj.position);

    for &child_idx in &obj.children {
        write_object(writer, level, child_idx);
    }
}

fn write_vector2(writer: &mut BinaryWriter, v: &Vec2) {
    writer.write_single(v.x);
    writer.write_single(v.y);
}

fn write_vector3(writer: &mut BinaryWriter, v: &Vec3) {
    writer.write_single(v.x);
    writer.write_single(v.y);
    writer.write_single(v.z);
}

fn write_color(writer: &mut BinaryWriter, c: &Color) {
    writer.write_uint32(c.to_packed());
}

fn write_mesh(writer: &mut BinaryWriter, mesh: &TerrainMesh) {
    writer.write_int32(mesh.vertices.len() as i32);
    for v in &mesh.vertices {
        write_vector2(writer, v);
    }
    writer.write_int32(mesh.indices.len() as i32);
    for &idx in &mesh.indices {
        writer.write_int16(idx);
    }
}

fn write_terrain(writer: &mut BinaryWriter, t: &TerrainData) {
    writer.write_single(t.fill_texture_tile_offset_x);
    writer.write_single(t.fill_texture_tile_offset_y);
    write_mesh(writer, &t.fill_mesh);
    write_color(writer, &t.fill_color);
    writer.write_int32(t.fill_texture_index);
    write_mesh(writer, &t.curve_mesh);

    writer.write_int32(t.curve_textures.len() as i32);
    for ct in &t.curve_textures {
        writer.write_int32(ct.texture_index);
        write_vector2(writer, &ct.size);
        writer.write_boolean(ct.fixed_angle);
        writer.write_single(ct.fade_threshold);
    }

    writer.write_int32(t.control_texture_count);
    if t.control_texture_count > 0
        && let Some(ref data) = t.control_texture_data
    {
        writer.write_int32(data.len() as i32);
        writer.write_bytes(data);
    }

    writer.write_boolean(t.has_collider);
}

fn write_prefab_overrides(writer: &mut BinaryWriter, data: &PrefabOverrideData) {
    writer.write_int32(data.raw_bytes.len() as i32);
    writer.write_bytes(&data.raw_bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    #[test]
    fn test_7bit_roundtrip() {
        let mut w = BinaryWriter::new();
        w.write_string("Hello, 世界!");
        let bytes = w.into_bytes();

        let mut r = BinaryReader::new(bytes);
        let s = r.read_string().unwrap();
        assert_eq!(s, "Hello, 世界!");
    }

    #[test]
    fn test_numeric_roundtrip() {
        let mut w = BinaryWriter::new();
        w.write_int16(-1234);
        w.write_int32(0x12345678);
        w.write_uint32(0xDEADBEEF);
        w.write_single(PI);
        w.write_boolean(true);
        w.write_boolean(false);
        let bytes = w.into_bytes();

        let mut r = BinaryReader::new(bytes);
        assert_eq!(r.read_int16().unwrap(), -1234);
        assert_eq!(r.read_int32().unwrap(), 0x12345678);
        assert_eq!(r.read_uint32().unwrap(), 0xDEADBEEF);
        assert!((r.read_single().unwrap() - PI).abs() < 0.001);
        assert!(r.read_boolean().unwrap());
        assert!(!r.read_boolean().unwrap());
    }

    #[test]
    fn test_color_roundtrip() {
        let c = Color {
            r: 1.0,
            g: 0.5,
            b: 0.25,
            a: 0.0,
        };
        let packed = c.to_packed();
        let c2 = Color::from_packed(packed);
        assert!((c.r - c2.r).abs() < 0.01);
        assert!((c.g - c2.g).abs() < 0.01);
        assert!((c.b - c2.b).abs() < 0.01);
        assert!((c.a - c2.a).abs() < 0.01);
    }

    #[test]
    fn test_level_roundtrip() {
        // Use a real level file for roundtrip testing
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../test_levels/assetbundles/episode_1_levels.unity3d/Level_05_data.bytes"
        );
        let data = std::fs::read(path).expect("test level file not found");
        let original = data.clone();

        let level = parse_level(data).expect("parse failed");
        assert!(!level.objects.is_empty(), "level should have objects");
        assert!(!level.roots.is_empty(), "level should have roots");

        // Re-serialize and verify byte-for-byte identity
        let reserialized = serialize_level(&level);
        assert_eq!(
            original.len(),
            reserialized.len(),
            "serialized length mismatch"
        );
        assert_eq!(original, reserialized, "roundtrip bytes mismatch");
    }
}
