use std::collections::HashMap;

use crate::data::unity_anim::HermiteKey;

const KNOWN_MONOBEHAVIOUR_GUID_SUFFIXES: &[(&str, &str)] = &[
    ("ae5f82fde6e6559b4e6280a34047fbb4", "PositionSerializer"),
    ("b0e2701ec6dd05b749c7483cb8824587", "Bridge"),
    ("5e1217a38f28f58b406d6bfa1d443506", "BezierCurve"),
    ("b0912da955fc22ac49cacf3bf45b15ff", "BezierMesh"),
    ("f800f9004829d2bf463a4d04b1b26c2c", "Engine"),
    ("136cea14b9e8b6964b418f8d2caa309d", "Fan"),
    ("1c0b17aff10189b24f7a3dc26453b419", "PointLightSource"),
    ("e8df771c7097e6a8496aaf8b005b6e19", "PartSecret"),
    ("5eaf56a364255ba8499cf075501b0d29", "Rocket"),
    ("d023fcf410bcc6ab493b676524aeacc2", "RopeVisualization"),
    ("eaa85264a31f76994888187c4d3a9fb9", "Sprite"),
    ("b011dfa16a4475b746a1372ea41fdf05", "UnmanagedSprite"),
    ("3d46d566866fd29148f73f2aa9b6b572", "WindArea"),
];

#[derive(Debug, Clone)]
pub struct PrefabAssetDocument {
    root_game_object_id: String,
    game_objects: HashMap<String, PrefabAssetGameObject>,
    transforms: HashMap<String, PrefabAssetTransform>,
    components: HashMap<String, PrefabAssetComponent>,
}

#[derive(Debug, Clone)]
struct PrefabAssetGameObject {
    name: String,
    component_ids: Vec<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct PrefabAssetTransform {
    pub game_object_id: String,
    pub local_pos: [f32; 3],
    pub local_rotation: [f32; 4],
    pub local_scale: [f32; 3],
    pub parent_id: Option<String>,
    pub children: Vec<String>,
    pub root_order: i32,
}

#[derive(Debug, Clone)]
pub struct PrefabAssetComponent {
    kind: PrefabAssetComponentKind,
    raw_doc: String,
}

#[derive(Debug, Clone)]
enum PrefabAssetComponentKind {
    Builtin(String),
    MonoBehaviour { script_guid: String },
}

impl PrefabAssetDocument {
    pub fn parse(text: &str) -> Option<Self> {
        let mut root_game_object_id = None;
        let mut game_objects = HashMap::new();
        let mut transforms = HashMap::new();
        let mut components = HashMap::new();

        for doc in text.split("--- ").skip(1) {
            let mut lines = doc.lines();
            let Some(header) = lines.next() else {
                continue;
            };
            let Some((class_id, file_id)) = parse_doc_header(header) else {
                continue;
            };

            match class_id {
                1001 => {
                    root_game_object_id =
                        field_value(doc, "m_RootGameObject: ").and_then(extract_file_id);
                }
                1 => {
                    let name = field_value(doc, "m_Name: ").unwrap_or(file_id).to_string();
                    let component_ids = parse_component_ids(doc);
                    game_objects.insert(
                        file_id.to_string(),
                        PrefabAssetGameObject {
                            name,
                            component_ids,
                        },
                    );
                }
                4 => {
                    let Some(game_object_id) =
                        field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                    else {
                        continue;
                    };
                    let local_pos = field_value(doc, "m_LocalPosition: ")
                        .and_then(parse_vec3)
                        .unwrap_or([0.0, 0.0, 0.0]);
                    let local_rotation = field_value(doc, "m_LocalRotation: ")
                        .and_then(parse_vec4)
                        .unwrap_or([0.0, 0.0, 0.0, 1.0]);
                    let local_scale = field_value(doc, "m_LocalScale: ")
                        .and_then(parse_vec3)
                        .unwrap_or([1.0, 1.0, 1.0]);
                    let parent_id = field_value(doc, "m_Father: ")
                        .and_then(extract_file_id)
                        .filter(|id| id != "0");
                    let root_order = field_value(doc, "m_RootOrder: ")
                        .and_then(|value| value.parse().ok())
                        .unwrap_or(0);
                    transforms.insert(
                        file_id.to_string(),
                        PrefabAssetTransform {
                            game_object_id: game_object_id.clone(),
                            local_pos,
                            local_rotation,
                            local_scale,
                            parent_id,
                            children: parse_children(doc),
                            root_order,
                        },
                    );
                    components.insert(
                        file_id.to_string(),
                        PrefabAssetComponent {
                            kind: PrefabAssetComponentKind::Builtin("Transform".to_string()),
                            raw_doc: doc.to_string(),
                        },
                    );
                }
                114 => {
                    let Some(game_object_id) =
                        field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                    else {
                        continue;
                    };
                    let Some(script_guid) = field_value(doc, "m_Script: ").and_then(extract_guid)
                    else {
                        continue;
                    };
                    components.insert(
                        file_id.to_string(),
                        PrefabAssetComponent {
                            kind: PrefabAssetComponentKind::MonoBehaviour {
                                script_guid: script_guid.to_string(),
                            },
                            raw_doc: doc.to_string(),
                        },
                    );
                    let _ = game_object_id;
                }
                _ => {
                    let Some(_game_object_id) =
                        field_value(doc, "m_GameObject: ").and_then(extract_file_id)
                    else {
                        continue;
                    };
                    let Some(component_name) = doc
                        .lines()
                        .nth(1)
                        .map(str::trim)
                        .and_then(|line| line.strip_suffix(':'))
                    else {
                        continue;
                    };
                    components.insert(
                        file_id.to_string(),
                        PrefabAssetComponent {
                            kind: PrefabAssetComponentKind::Builtin(component_name.to_string()),
                            raw_doc: doc.to_string(),
                        },
                    );
                }
            }
        }

        let root_game_object_id = root_game_object_id.or_else(|| {
            transforms
                .values()
                .find(|transform| transform.parent_id.is_none())
                .map(|transform| transform.game_object_id.clone())
        })?;

        Some(Self {
            root_game_object_id,
            game_objects,
            transforms,
            components,
        })
    }

    pub fn root_component_suffixes(&self) -> Vec<String> {
        let Some(root) = self.game_objects.get(&self.root_game_object_id) else {
            return Vec::new();
        };

        let mut suffixes = Vec::new();
        for component_id in &root.component_ids {
            let Some(suffix) = self
                .components
                .get(component_id)
                .and_then(PrefabAssetComponent::component_suffix)
            else {
                continue;
            };
            if !suffixes.iter().any(|existing| existing == suffix) {
                suffixes.push(suffix.to_string());
            }
        }

        suffixes
    }

    pub fn root_script_guids(&self) -> Vec<String> {
        let Some(root) = self.game_objects.get(&self.root_game_object_id) else {
            return Vec::new();
        };

        let mut guids = Vec::new();
        for component_id in &root.component_ids {
            let Some(script_guid) = self
                .components
                .get(component_id)
                .and_then(PrefabAssetComponent::script_guid)
            else {
                continue;
            };
            if !guids.iter().any(|existing| existing == script_guid) {
                guids.push(script_guid.to_string());
            }
        }

        guids
    }

    pub fn root_component(&self, suffix: &str) -> Option<&PrefabAssetComponent> {
        let root = self.game_objects.get(&self.root_game_object_id)?;

        root.component_ids
            .iter()
            .filter_map(|component_id| self.components.get(component_id))
            .find(|component| component.component_suffix() == Some(suffix))
    }

    pub fn component_by_game_object_name(
        &self,
        game_object_name: &str,
        suffix: &str,
    ) -> Option<&PrefabAssetComponent> {
        let game_object_id =
            self.game_objects
                .iter()
                .find_map(|(game_object_id, game_object)| {
                    (game_object.name == game_object_name).then_some(game_object_id.as_str())
                })?;

        self.game_objects
            .get(game_object_id)?
            .component_ids
            .iter()
            .filter_map(|component_id| self.components.get(component_id))
            .find(|component| component.component_suffix() == Some(suffix))
    }

    pub fn root_transform(&self) -> Option<&PrefabAssetTransform> {
        self.transforms
            .values()
            .find(|transform| transform.game_object_id == self.root_game_object_id)
    }

    pub fn transform_by_game_object_name(&self, name: &str) -> Option<&PrefabAssetTransform> {
        let game_object_id =
            self.game_objects
                .iter()
                .find_map(|(game_object_id, game_object)| {
                    (game_object.name == name).then_some(game_object_id.as_str())
                })?;

        self.transforms
            .values()
            .find(|transform| transform.game_object_id == game_object_id)
    }

    pub fn cumulative_scale_by_game_object_name(&self, name: &str) -> Option<[f32; 3]> {
        let transform_id = self
            .transforms
            .iter()
            .find_map(|(transform_id, transform)| {
                let game_object = self.game_objects.get(&transform.game_object_id)?;
                (game_object.name == name).then_some(transform_id.as_str())
            })?;

        let mut scale = [1.0, 1.0, 1.0];
        let mut current_id = Some(transform_id.to_string());

        while let Some(transform_id) = current_id {
            let transform = self.transforms.get(&transform_id)?;
            scale[0] *= transform.local_scale[0];
            scale[1] *= transform.local_scale[1];
            scale[2] *= transform.local_scale[2];
            current_id = transform.parent_id.clone();
        }

        Some(scale)
    }

    /// Sum of `m_LocalPosition` along the chain from the named GameObject up to
    /// (but excluding) the prefab root. This gives the offset relative to the
    /// prefab root, ignoring the root's own scene placement. Assumes intermediate
    /// transforms have zero rotation (true for all compound part prefabs).
    pub fn cumulative_local_pos_by_game_object_name(&self, name: &str) -> Option<[f32; 3]> {
        let transform_id = self
            .transforms
            .iter()
            .find_map(|(transform_id, transform)| {
                let game_object = self.game_objects.get(&transform.game_object_id)?;
                (game_object.name == name).then_some(transform_id.as_str())
            })?;

        let mut pos = [0.0, 0.0, 0.0];
        let mut current_id = Some(transform_id.to_string());

        while let Some(id) = current_id {
            let transform = self.transforms.get(&id)?;
            // Skip the prefab root's own local position (that's its scene placement,
            // not part of the in-prefab layout).
            if transform.parent_id.is_none() {
                break;
            }
            pos[0] += transform.local_pos[0];
            pos[1] += transform.local_pos[1];
            pos[2] += transform.local_pos[2];
            current_id = transform.parent_id.clone();
        }

        Some(pos)
    }

    /// Find a transform by walking a slash-separated path of GameObject names starting
    /// at the prefab root. e.g. `"Button/Graphics"` returns the Graphics child of
    /// the Button child of the root. The path does NOT include the root's own name.
    pub fn transform_by_path(&self, path: &str) -> Option<&PrefabAssetTransform> {
        let mut current_transform_id: Option<String> = self
            .transforms
            .iter()
            .find(|(_, transform)| transform.parent_id.is_none())
            .map(|(id, _)| id.clone());

        for segment in path.split('/').filter(|s| !s.is_empty()) {
            let parent_id = current_transform_id.as_ref()?;
            let parent = self.transforms.get(parent_id)?;
            let mut found = None;
            for child_id in &parent.children {
                let child = self.transforms.get(child_id)?;
                let go = self.game_objects.get(&child.game_object_id)?;
                if go.name == segment {
                    found = Some(child_id.clone());
                    break;
                }
            }
            current_transform_id = found;
        }

        current_transform_id.and_then(|id| self.transforms.get(&id))
    }

    /// Component lookup by slash-separated path (see `transform_by_path`).
    pub fn component_by_path(&self, path: &str, suffix: &str) -> Option<&PrefabAssetComponent> {
        let transform = self.transform_by_path(path)?;
        let game_object = self.game_objects.get(&transform.game_object_id)?;
        game_object
            .component_ids
            .iter()
            .filter_map(|component_id| self.components.get(component_id))
            .find(|component| component.component_suffix() == Some(suffix))
    }

    /// Cumulative local position along a slash-separated path (root is excluded).
    pub fn cumulative_local_pos_by_path(&self, path: &str) -> Option<[f32; 3]> {
        let mut pos = [0.0, 0.0, 0.0];
        let mut current_transform_id: Option<String> = self
            .transforms
            .iter()
            .find(|(_, transform)| transform.parent_id.is_none())
            .map(|(id, _)| id.clone());

        for segment in path.split('/').filter(|s| !s.is_empty()) {
            let parent_id = current_transform_id.as_ref()?;
            let parent = self.transforms.get(parent_id)?;
            let mut found = None;
            for child_id in &parent.children {
                let child = self.transforms.get(child_id)?;
                let go = self.game_objects.get(&child.game_object_id)?;
                if go.name == segment {
                    pos[0] += child.local_pos[0];
                    pos[1] += child.local_pos[1];
                    pos[2] += child.local_pos[2];
                    found = Some(child_id.clone());
                    break;
                }
            }
            current_transform_id = found;
        }
        current_transform_id?;
        Some(pos)
    }

    /// Cumulative local scale along a slash-separated path. Root scale is EXCLUDED
    /// (the root's transform represents the prefab's scene placement, not its
    /// internal geometry).
    pub fn cumulative_local_scale_by_path(&self, path: &str) -> Option<[f32; 3]> {
        let mut scale = [1.0, 1.0, 1.0];
        let root_id = self
            .transforms
            .iter()
            .find(|(_, transform)| transform.parent_id.is_none())
            .map(|(id, _)| id.clone())?;

        let mut current_transform_id: Option<String> = Some(root_id);
        for segment in path.split('/').filter(|s| !s.is_empty()) {
            let parent_id = current_transform_id.as_ref()?;
            let parent = self.transforms.get(parent_id)?;
            let mut found = None;
            for child_id in &parent.children {
                let child = self.transforms.get(child_id)?;
                let go = self.game_objects.get(&child.game_object_id)?;
                if go.name == segment {
                    scale[0] *= child.local_scale[0];
                    scale[1] *= child.local_scale[1];
                    scale[2] *= child.local_scale[2];
                    found = Some(child_id.clone());
                    break;
                }
            }
            current_transform_id = found;
        }
        current_transform_id?;
        Some(scale)
    }

    /// Transform a local point on the GameObject at `path` into prefab-root local
    /// space. The prefab root's own scene-placement transform is excluded.
    pub fn transform_point_by_path(&self, path: &str, point: [f32; 3]) -> Option<[f32; 3]> {
        let transform = self.transform_by_path(path)?;
        self.transform_point_by_file_id(
            self.transforms.iter().find_map(|(id, candidate)| {
                (candidate.game_object_id == transform.game_object_id
                    && candidate.local_pos == transform.local_pos
                    && candidate.local_rotation == transform.local_rotation
                    && candidate.local_scale == transform.local_scale
                    && candidate.parent_id == transform.parent_id)
                    .then_some(id.as_str())
            })?,
            point,
        )
    }

    /// Transform a local point on the transform identified by `file_id` into
    /// prefab-root local space. The prefab root's own scene-placement transform is
    /// excluded.
    pub fn transform_point_by_file_id(&self, file_id: &str, point: [f32; 3]) -> Option<[f32; 3]> {
        let mut transformed = point;
        let mut current_id = Some(file_id.to_string());

        while let Some(transform_id) = current_id {
            let transform = self.transforms.get(&transform_id)?;
            if transform.parent_id.is_none() {
                break;
            }
            transformed = transform.transform_point(transformed);
            current_id = transform.parent_id.clone();
        }

        Some(transformed)
    }
}

impl PrefabAssetTransform {
    fn transform_point(&self, point: [f32; 3]) -> [f32; 3] {
        let scaled = [
            point[0] * self.local_scale[0],
            point[1] * self.local_scale[1],
            point[2] * self.local_scale[2],
        ];
        let rotated = rotate_vec3(self.local_rotation, scaled);
        [
            rotated[0] + self.local_pos[0],
            rotated[1] + self.local_pos[1],
            rotated[2] + self.local_pos[2],
        ]
    }
}

impl PrefabAssetComponent {
    pub fn script_guid(&self) -> Option<&str> {
        match &self.kind {
            PrefabAssetComponentKind::Builtin(_) => None,
            PrefabAssetComponentKind::MonoBehaviour { script_guid } => Some(script_guid.as_str()),
        }
    }

    pub fn component_suffix(&self) -> Option<&str> {
        match &self.kind {
            PrefabAssetComponentKind::Builtin(name) => Some(name.as_str()),
            PrefabAssetComponentKind::MonoBehaviour { script_guid } => {
                mono_behaviour_suffix(script_guid)
            }
        }
    }

    pub fn field_f32(&self, field_name: &str) -> Option<f32> {
        self.field_value(field_name)?.parse().ok()
    }

    pub fn field_i32(&self, field_name: &str) -> Option<i32> {
        self.field_value(field_name)?.parse().ok()
    }

    pub fn field_vec3(&self, field_name: &str) -> Option<[f32; 3]> {
        parse_vec3(self.field_value(field_name)?)
    }

    pub fn field_curve(&self, field_name: &str) -> Option<Vec<HermiteKey>> {
        let field_header = format!("\n  {}:\n", field_name);
        let field_start = self.raw_doc.find(&field_header)? + field_header.len();
        let curve_marker = "    m_Curve:\n";
        let curve_relative_start = self.raw_doc[field_start..].find(curve_marker)?;
        let curve_start = field_start + curve_relative_start + curve_marker.len();

        let mut keys = Vec::new();
        let mut current_time = None;
        let mut current_value = None;
        let mut current_in_slope = None;
        let mut current_out_slope = None;

        for line in self.raw_doc[curve_start..].lines() {
            if line.starts_with("    - ") {
                if let Some(key) = build_curve_key(
                    current_time,
                    current_value,
                    current_in_slope,
                    current_out_slope,
                ) {
                    keys.push(key);
                }
                current_time = None;
                current_value = None;
                current_in_slope = None;
                current_out_slope = None;
                continue;
            }

            if let Some(value) = line.strip_prefix("      time: ") {
                current_time = value.parse().ok();
                continue;
            }
            if let Some(value) = line.strip_prefix("      value: ") {
                current_value = value.parse().ok();
                continue;
            }
            if let Some(value) = line.strip_prefix("      inSlope: ") {
                current_in_slope = value.parse().ok();
                continue;
            }
            if let Some(value) = line.strip_prefix("      outSlope: ") {
                current_out_slope = value.parse().ok();
                continue;
            }

            if !line.starts_with("      ") {
                break;
            }
        }

        if let Some(key) = build_curve_key(
            current_time,
            current_value,
            current_in_slope,
            current_out_slope,
        ) {
            keys.push(key);
        }

        if keys.is_empty() {
            return None;
        }

        Some(keys)
    }

    pub fn field_bool(&self, field_name: &str) -> Option<bool> {
        match self.field_value(field_name)? {
            "1" | "True" | "true" => Some(true),
            "0" | "False" | "false" => Some(false),
            _ => None,
        }
    }

    pub fn field_guid(&self, field_name: &str) -> Option<&str> {
        if let Some(value) = self.field_value(field_name)
            && let Some(guid) = extract_guid(value)
        {
            return Some(guid);
        }

        let field_header = format!("\n  {}:\n", field_name);
        let field_start = self.raw_doc.find(&field_header)? + field_header.len();

        for line in self.raw_doc[field_start..].lines() {
            if !line.starts_with("  ") {
                break;
            }
            if let Some(guid) = extract_guid(line.trim()) {
                return Some(guid);
            }
        }

        None
    }

    pub fn field_file_id(&self, field_name: &str) -> Option<String> {
        if let Some(value) = self.field_value(field_name)
            && let Some(file_id) = extract_file_id(value)
        {
            return Some(file_id);
        }

        let field_header = format!("\n  {}:\n", field_name);
        let field_start = self.raw_doc.find(&field_header)? + field_header.len();

        for line in self.raw_doc[field_start..].lines() {
            if !line.starts_with("  ") {
                break;
            }
            if let Some(file_id) = extract_file_id(line.trim()) {
                return Some(file_id);
            }
        }

        None
    }

    fn field_value(&self, field_name: &str) -> Option<&str> {
        let prefix = format!("{field_name}: ");
        field_value(&self.raw_doc, &prefix)
    }

    /// Public accessor for a single-line scalar field's raw string value.
    pub fn field_str(&self, field_name: &str) -> Option<&str> {
        self.field_value(field_name)
    }
}

pub fn mono_behaviour_suffix(guid: &str) -> Option<&'static str> {
    KNOWN_MONOBEHAVIOUR_GUID_SUFFIXES
        .iter()
        .find_map(|(known_guid, suffix)| (*known_guid == guid).then_some(*suffix))
}

fn field_value<'a>(doc: &'a str, prefix: &str) -> Option<&'a str> {
    doc.lines()
        .find_map(|line| line.trim().strip_prefix(prefix).map(str::trim))
}

fn parse_doc_header(header: &str) -> Option<(u32, &str)> {
    let rest = header.trim().strip_prefix("!u!")?;
    let (type_id, file_id) = rest.split_once(" &")?;
    Some((type_id.parse().ok()?, file_id.trim()))
}

fn extract_file_id(value: &str) -> Option<String> {
    let start = value.find("fileID: ")? + "fileID: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    let file_id = tail[..end].trim();
    (!file_id.is_empty()).then(|| file_id.to_string())
}

fn extract_guid(value: &str) -> Option<&str> {
    let start = value.find("guid: ")? + "guid: ".len();
    let tail = &value[start..];
    let end = tail.find(|c| [',', '}'].contains(&c)).unwrap_or(tail.len());
    let guid = tail[..end].trim();
    (!guid.is_empty()).then_some(guid)
}

fn build_curve_key(
    time: Option<f32>,
    value: Option<f32>,
    in_slope: Option<f32>,
    out_slope: Option<f32>,
) -> Option<HermiteKey> {
    Some((time?, value?, in_slope?, out_slope?))
}

fn parse_vec3(value: &str) -> Option<[f32; 3]> {
    let mut out = [0.0; 3];
    let trimmed = value.trim().trim_start_matches('{').trim_end_matches('}');
    let mut seen = [false; 3];
    for part in trimmed.split(',') {
        let (axis, raw) = part.trim().split_once(':')?;
        let index = match axis.trim() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            _ => continue,
        };
        out[index] = raw.trim().parse().ok()?;
        seen[index] = true;
    }
    seen.iter().all(|value| *value).then_some(out)
}

fn parse_vec4(value: &str) -> Option<[f32; 4]> {
    let mut out = [0.0; 4];
    let trimmed = value.trim().trim_start_matches('{').trim_end_matches('}');
    let mut seen = [false; 4];
    for part in trimmed.split(',') {
        let (axis, raw) = part.trim().split_once(':')?;
        let index = match axis.trim() {
            "x" => 0,
            "y" => 1,
            "z" => 2,
            "w" => 3,
            _ => continue,
        };
        out[index] = raw.trim().parse().ok()?;
        seen[index] = true;
    }
    seen.iter().all(|value| *value).then_some(out)
}

fn rotate_vec3(quat: [f32; 4], point: [f32; 3]) -> [f32; 3] {
    let [qx, qy, qz, qw] = quat;
    let uv = [
        qy * point[2] - qz * point[1],
        qz * point[0] - qx * point[2],
        qx * point[1] - qy * point[0],
    ];
    let uuv = [
        qy * uv[2] - qz * uv[1],
        qz * uv[0] - qx * uv[2],
        qx * uv[1] - qy * uv[0],
    ];
    [
        point[0] + 2.0 * (qw * uv[0] + uuv[0]),
        point[1] + 2.0 * (qw * uv[1] + uuv[1]),
        point[2] + 2.0 * (qw * uv[2] + uuv[2]),
    ]
}

fn parse_component_ids(doc: &str) -> Vec<String> {
    let mut component_ids = Vec::new();
    let mut in_components = false;
    for line in doc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("m_Component:") {
            in_components = true;
            continue;
        }
        if !in_components {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(component_id) = extract_file_id(rest) {
                component_ids.push(component_id);
            }
            continue;
        }
        if !trimmed.is_empty() {
            break;
        }
    }
    component_ids
}

fn parse_children(doc: &str) -> Vec<String> {
    let mut children = Vec::new();
    let mut in_children = false;
    for line in doc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("m_Children:") {
            in_children = true;
            continue;
        }
        if !in_children {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("- ") {
            if let Some(child_id) = extract_file_id(rest) {
                children.push(child_id);
            }
            continue;
        }
        if trimmed.starts_with("m_Father:") {
            break;
        }
        if !trimmed.is_empty() {
            break;
        }
    }
    children
}

#[cfg(test)]
mod tests {
    use super::{PrefabAssetDocument, mono_behaviour_suffix};

    const SAMPLE_PREFAB: &str = "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1001 &100100000\nPrefab:\n  m_RootGameObject: {fileID: 101}\n--- !u!1 &101\nGameObject:\n  m_Component:\n  - component: {fileID: 201}\n  - component: {fileID: 202}\n  - component: {fileID: 203}\n  - component: {fileID: 204}\n  m_Name: Bridge\n--- !u!4 &201\nTransform:\n  m_GameObject: {fileID: 101}\n  m_LocalPosition: {x: 0, y: 0, z: 0}\n  m_LocalScale: {x: 1, y: 1, z: 1}\n  m_Children:\n  - {fileID: 301}\n  m_Father: {fileID: 0}\n  m_RootOrder: 0\n--- !u!114 &202\nMonoBehaviour:\n  m_GameObject: {fileID: 101}\n  m_Script: {fileID: 11500000, guid: b0e2701ec6dd05b749c7483cb8824587, type: 3}\n  stepLength: 1\n  stepGap: 0.2\n  verticalRamp:\n    serializedVersion: 2\n    m_Curve:\n    - serializedVersion: 2\n      time: 0\n      value: 0\n      inSlope: 0\n      outSlope: 0\n      tangentMode: 0\n    - serializedVersion: 2\n      time: 1\n      value: 1\n      inSlope: 2\n      outSlope: 2\n      tangentMode: 0\n--- !u!65 &203\nBoxCollider:\n  m_GameObject: {fileID: 101}\n  m_Size: {x: 40, y: 15, z: 10}\n--- !u!23 &204\nMeshRenderer:\n  m_GameObject: {fileID: 101}\n  m_Materials:\n  - {fileID: 2100000, guid: ce5a9931cec8f4b84741e1391306eb66, type: 2}\n--- !u!1 &111\nGameObject:\n  m_Component:\n  - component: {fileID: 301}\n  m_Name: EndPoint\n--- !u!4 &301\nTransform:\n  m_GameObject: {fileID: 111}\n  m_LocalPosition: {x: 2.561546, y: 0, z: 0}\n  m_LocalScale: {x: 1, y: 1, z: 1}\n  m_Children: []\n  m_Father: {fileID: 201}\n  m_RootOrder: 0\n";

    const TRANSFORM_POINT_SAMPLE_PREFAB: &str = "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1001 &100100000\nPrefab:\n  m_RootGameObject: {fileID: 1}\n--- !u!1 &1\nGameObject:\n  m_Component:\n  - component: {fileID: 10}\n  m_Name: Root\n--- !u!4 &10\nTransform:\n  m_GameObject: {fileID: 1}\n  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}\n  m_LocalPosition: {x: 0, y: 0, z: 0}\n  m_LocalScale: {x: 1, y: 1, z: 1}\n  m_Children:\n  - {fileID: 20}\n  m_Father: {fileID: 0}\n  m_RootOrder: 0\n--- !u!1 &2\nGameObject:\n  m_Component:\n  - component: {fileID: 20}\n  m_Name: Child\n--- !u!4 &20\nTransform:\n  m_GameObject: {fileID: 2}\n  m_LocalRotation: {x: 0, y: 0, z: 0.70710677, w: 0.70710677}\n  m_LocalPosition: {x: 4, y: 5, z: 0}\n  m_LocalScale: {x: 2, y: 3, z: 1}\n  m_Children:\n  - {fileID: 30}\n  m_Father: {fileID: 10}\n  m_RootOrder: 0\n--- !u!1 &3\nGameObject:\n  m_Component:\n  - component: {fileID: 30}\n  m_Name: Grandchild\n--- !u!4 &30\nTransform:\n  m_GameObject: {fileID: 3}\n  m_LocalRotation: {x: 0, y: 0, z: 0, w: 1}\n  m_LocalPosition: {x: 1, y: 0, z: 0}\n  m_LocalScale: {x: 1, y: 1, z: 1}\n  m_Children: []\n  m_Father: {fileID: 20}\n  m_RootOrder: 0\n";

    #[test]
    fn parses_root_components_and_named_transform_defaults() {
        let prefab = PrefabAssetDocument::parse(SAMPLE_PREFAB).expect("expected prefab");
        let bridge = prefab
            .root_component("Bridge")
            .expect("missing Bridge component");
        let bridge_by_name = prefab
            .component_by_game_object_name("Bridge", "Bridge")
            .expect("missing Bridge component on Bridge GameObject");
        let collider = prefab
            .root_component("BoxCollider")
            .expect("missing BoxCollider component");
        let mesh_renderer = prefab
            .component_by_game_object_name("Bridge", "MeshRenderer")
            .expect("missing MeshRenderer component on Bridge GameObject");
        let root = prefab.root_transform().expect("missing root transform");
        let endpoint = prefab
            .transform_by_game_object_name("EndPoint")
            .expect("missing EndPoint transform");

        assert_eq!(
            prefab.root_component_suffixes(),
            vec!["Transform", "Bridge", "BoxCollider", "MeshRenderer"]
        );
        assert_eq!(bridge.field_f32("stepLength"), Some(1.0));
        assert_eq!(bridge_by_name.field_f32("stepLength"), Some(1.0));
        assert_eq!(bridge.field_i32("stepLength"), Some(1));
        assert_eq!(
            mesh_renderer.field_guid("m_Materials"),
            Some("ce5a9931cec8f4b84741e1391306eb66")
        );
        assert_eq!(bridge.field_f32("stepGap"), Some(0.2));
        assert_eq!(
            bridge.field_curve("verticalRamp"),
            Some(vec![(0.0, 0.0, 0.0, 0.0), (1.0, 1.0, 2.0, 2.0)])
        );
        assert_eq!(collider.field_vec3("m_Size"), Some([40.0, 15.0, 10.0]));
        assert_eq!(root.local_pos, [0.0, 0.0, 0.0]);
        assert_eq!(endpoint.local_pos, [2.561546, 0.0, 0.0]);
    }

    #[test]
    fn transforms_points_through_scaled_and_rotated_children() {
        let prefab =
            PrefabAssetDocument::parse(TRANSFORM_POINT_SAMPLE_PREFAB).expect("expected prefab");

        assert_eq!(
            prefab.transform_point_by_path("Child", [1.0, 0.0, 0.0]),
            Some([4.0, 7.0, 0.0])
        );
        assert_eq!(
            prefab.transform_point_by_path("Child/Grandchild", [0.0, 0.0, 0.0]),
            Some([4.0, 7.0, 0.0])
        );
        assert_eq!(
            prefab.transform_point_by_file_id("20", [1.0, 0.0, 0.0]),
            Some([4.0, 7.0, 0.0])
        );
        assert_eq!(
            prefab.transform_point_by_file_id("10", [1.0, 2.0, 0.0]),
            Some([1.0, 2.0, 0.0])
        );
    }

    #[test]
    fn recognizes_common_goal_prefab_sprite_behaviours() {
        assert_eq!(
            mono_behaviour_suffix("eaa85264a31f76994888187c4d3a9fb9"),
            Some("Sprite")
        );
        assert_eq!(
            mono_behaviour_suffix("b011dfa16a4475b746a1372ea41fdf05"),
            Some("UnmanagedSprite")
        );
    }
}
