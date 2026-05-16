//! Typed runtime model for Unity ObjectDeserializer override text.

use crate::domain::prefab_override::{OverrideNode, parse_override_text};
use crate::domain::types::{Vec2, Vec3};

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOverrideDocument {
    pub roots: Vec<RuntimeOverrideNode>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOverrideNode {
    pub node_type: String,
    pub name: String,
    pub value: RuntimeOverrideValue,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RuntimeOverrideValue {
    Empty,
    Integer(i32),
    Float(f32),
    String(String),
    Boolean(bool),
    Enum(i32),
    ObjectReference(i32),
    Struct(Vec<RuntimeOverrideNode>),
    Array(RuntimeOverrideArray),
    Raw(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOverrideArray {
    pub size: Option<usize>,
    pub elements: Vec<RuntimeOverrideArrayElement>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimeOverrideArrayElement {
    pub index: usize,
    pub value: RuntimeOverrideNode,
}

impl RuntimeOverrideDocument {
    pub fn parse(raw: &str) -> Self {
        Self::from_nodes(&parse_override_text(raw))
    }

    pub fn from_nodes(nodes: &[OverrideNode]) -> Self {
        Self {
            roots: nodes.iter().map(RuntimeOverrideNode::from_ast).collect(),
        }
    }

    pub fn root(&self, node_type: &str, name: &str) -> Option<&RuntimeOverrideNode> {
        self.roots
            .iter()
            .find(|node| node.node_type == node_type && node.name == name)
    }

    pub fn roots_of_type<'a>(
        &'a self,
        node_type: &'a str,
    ) -> impl Iterator<Item = &'a RuntimeOverrideNode> + 'a {
        self.roots
            .iter()
            .filter(move |node| node.node_type == node_type)
    }
}

impl RuntimeOverrideNode {
    pub fn child(&self, node_type: &str, name: &str) -> Option<&RuntimeOverrideNode> {
        self.children()
            .iter()
            .find(|node| node.node_type == node_type && node.name == name)
    }

    pub fn children_of_type<'a>(
        &'a self,
        node_type: &'a str,
    ) -> impl Iterator<Item = &'a RuntimeOverrideNode> + 'a {
        self.children()
            .iter()
            .filter(move |node| node.node_type == node_type)
    }

    pub fn children(&self) -> &[RuntimeOverrideNode] {
        match &self.value {
            RuntimeOverrideValue::Struct(children) => children,
            _ => &[],
        }
    }

    pub fn component(&self, suffix: &str) -> Option<&RuntimeOverrideNode> {
        self.children().iter().find(|node| {
            node.node_type == "Component"
                && (node.name == suffix
                    || node
                        .name
                        .rsplit('.')
                        .next()
                        .is_some_and(|name| name == suffix))
        })
    }

    pub fn find_descendant<F>(&self, predicate: &F) -> Option<&RuntimeOverrideNode>
    where
        F: Fn(&RuntimeOverrideNode) -> bool,
    {
        if predicate(self) {
            return Some(self);
        }

        self.children()
            .iter()
            .find_map(|child| child.find_descendant(predicate))
    }

    pub fn as_array(&self) -> Option<&RuntimeOverrideArray> {
        match &self.value {
            RuntimeOverrideValue::Array(array) => Some(array),
            _ => None,
        }
    }

    pub fn value_as_f32(&self) -> Option<f32> {
        match &self.value {
            RuntimeOverrideValue::Float(value) => Some(*value),
            RuntimeOverrideValue::Integer(value) => Some(*value as f32),
            _ => None,
        }
    }

    pub fn value_as_i32(&self) -> Option<i32> {
        match &self.value {
            RuntimeOverrideValue::Integer(value)
            | RuntimeOverrideValue::Enum(value)
            | RuntimeOverrideValue::ObjectReference(value) => Some(*value),
            _ => None,
        }
    }

    pub fn value_as_bool(&self) -> Option<bool> {
        match &self.value {
            RuntimeOverrideValue::Boolean(value) => Some(*value),
            _ => None,
        }
    }

    pub fn value_as_str(&self) -> Option<&str> {
        match &self.value {
            RuntimeOverrideValue::String(value) | RuntimeOverrideValue::Raw(value) => Some(value),
            _ => None,
        }
    }

    pub fn value_as_vec2(&self) -> Option<Vec2> {
        Some(Vec2 {
            x: self.child("Float", "x")?.value_as_f32()?,
            y: self.child("Float", "y")?.value_as_f32()?,
        })
    }

    pub fn value_as_vec3(&self) -> Option<Vec3> {
        let mut value = Vec3::default();
        let mut seen = false;

        for child in self.children() {
            let Some(component) = child.value_as_f32() else {
                continue;
            };

            match child.name.as_str() {
                "x" => {
                    value.x = component;
                    seen = true;
                }
                "y" => {
                    value.y = component;
                    seen = true;
                }
                "z" => {
                    value.z = component;
                    seen = true;
                }
                _ => {}
            }
        }

        seen.then_some(value)
    }

    pub fn partial_vec3(&self) -> [Option<f32>; 3] {
        let mut value = [None, None, None];

        for child in self.children() {
            let Some(component) = child.value_as_f32() else {
                continue;
            };

            match child.name.as_str() {
                "x" => value[0] = Some(component),
                "y" => value[1] = Some(component),
                "z" => value[2] = Some(component),
                _ => {}
            }
        }

        value
    }

    pub fn value_as_quaternion(&self) -> Option<[f32; 4]> {
        let mut value = [0.0; 4];
        let mut seen = false;

        for child in self.children() {
            let Some(component) = child.value_as_f32() else {
                continue;
            };

            match child.name.as_str() {
                "x" => {
                    value[0] = component;
                    seen = true;
                }
                "y" => {
                    value[1] = component;
                    seen = true;
                }
                "z" => {
                    value[2] = component;
                    seen = true;
                }
                "w" => {
                    value[3] = component;
                    seen = true;
                }
                _ => {}
            }
        }

        seen.then_some(value)
    }

    fn from_ast(node: &OverrideNode) -> Self {
        let value = match node.node_type.as_str() {
            "Integer" => node
                .value
                .as_deref()
                .and_then(|value| value.parse::<i32>().ok())
                .map(RuntimeOverrideValue::Integer)
                .unwrap_or_else(|| {
                    RuntimeOverrideValue::Raw(node.value.clone().unwrap_or_default())
                }),
            "Float" => node
                .value
                .as_deref()
                .and_then(|value| value.parse::<f32>().ok())
                .map(RuntimeOverrideValue::Float)
                .unwrap_or_else(|| {
                    RuntimeOverrideValue::Raw(node.value.clone().unwrap_or_default())
                }),
            "String" => node
                .value
                .clone()
                .map(RuntimeOverrideValue::String)
                .unwrap_or(RuntimeOverrideValue::Empty),
            "Boolean" => node
                .value
                .as_deref()
                .and_then(|value| {
                    if value.eq_ignore_ascii_case("true") {
                        Some(true)
                    } else if value.eq_ignore_ascii_case("false") {
                        Some(false)
                    } else {
                        None
                    }
                })
                .map(RuntimeOverrideValue::Boolean)
                .unwrap_or_else(|| {
                    RuntimeOverrideValue::Raw(node.value.clone().unwrap_or_default())
                }),
            "Enum" => node
                .value
                .as_deref()
                .and_then(|value| value.parse::<i32>().ok())
                .map(RuntimeOverrideValue::Enum)
                .unwrap_or_else(|| {
                    RuntimeOverrideValue::Raw(node.value.clone().unwrap_or_default())
                }),
            "ObjectReference" => node
                .value
                .as_deref()
                .and_then(|value| value.parse::<i32>().ok())
                .map(RuntimeOverrideValue::ObjectReference)
                .unwrap_or_else(|| {
                    RuntimeOverrideValue::Raw(node.value.clone().unwrap_or_default())
                }),
            "Array" => RuntimeOverrideValue::Array(RuntimeOverrideArray::from_children(&node.children)),
            _ if !node.children.is_empty() => RuntimeOverrideValue::Struct(
                node.children.iter().map(RuntimeOverrideNode::from_ast).collect(),
            ),
            _ => node
                .value
                .clone()
                .map(RuntimeOverrideValue::Raw)
                .unwrap_or(RuntimeOverrideValue::Empty),
        };

        Self {
            node_type: node.node_type.clone(),
            name: node.name.clone(),
            value,
        }
    }
}

impl RuntimeOverrideArray {
    pub fn element(&self, index: usize) -> Option<&RuntimeOverrideNode> {
        self.elements
            .iter()
            .find(|element| element.index == index)
            .map(|element| &element.value)
    }

    pub fn iter(&self) -> impl Iterator<Item = &RuntimeOverrideArrayElement> {
        self.elements.iter()
    }

    fn from_children(children: &[OverrideNode]) -> Self {
        let size = children
            .iter()
            .find(|child| child.node_type == "ArraySize" && child.name == "size")
            .and_then(|child| child.value.as_deref())
            .and_then(|value| value.parse::<usize>().ok());

        let elements = children
            .iter()
            .filter(|child| child.node_type == "Element")
            .filter_map(|child| {
                let index = child.name.parse::<usize>().ok()?;
                Some(RuntimeOverrideArrayElement {
                    index,
                    value: array_element_payload(child),
                })
            })
            .collect();

        Self { size, elements }
    }
}

fn array_element_payload(node: &OverrideNode) -> RuntimeOverrideNode {
    if node.children.len() == 1 {
        RuntimeOverrideNode::from_ast(&node.children[0])
    } else if let Some((head, tail)) = node.children.split_first()
        && head.value.is_none()
        && is_value_type_marker(&head.node_type)
    {
        let mut merged = head.clone();
        merged.children.extend_from_slice(tail);
        RuntimeOverrideNode::from_ast(&merged)
    } else {
        RuntimeOverrideNode {
            node_type: "ElementData".to_string(),
            name: node.name.clone(),
            value: RuntimeOverrideValue::Struct(
                node.children.iter().map(RuntimeOverrideNode::from_ast).collect(),
            ),
        }
    }
}

fn is_value_type_marker(node_type: &str) -> bool {
    matches!(
        node_type,
        "Generic"
            | "Color"
            | "Vector2"
            | "Vector3"
            | "Rect"
            | "Bounds"
            | "16"
            | "Quaternion"
            | "Keyframe"
    )
}

#[cfg(test)]
mod tests {
    use super::RuntimeOverrideDocument;

    const WINDAREA_OVERRIDE: &str = "GameObject WindArea\n\tComponent UnityEngine.BoxCollider\n\t\tVector3 m_Size\n\t\t\tFloat x = 31.4\n\tComponent WindArea\n\t\tVector3 windDirectionHandle\n\t\t\tFloat x = 17.67106\n\t\t\tFloat y = 0.617309\n\t\t\tFloat z = 0\n\t\tFloat m_windPowerFactor = 0.26\n\tGameObject WindEffect1\n\t\tComponent UnityEngine.Transform\n\t\t\tQuaternion m_LocalRotation\n\t\t\t\tFloat x = 0.0005903003\n\t\t\t\tFloat y = 0.7071065\n\t\t\t\tFloat z = -0.0005903003\n\t\t\t\tFloat w = 0.7071065\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = -15.69998\n\t\t\t\tFloat y = 0.01111133\n\t\t\t\tFloat z = -2\n\t\tComponent UnityEngine.ParticleSystem\n\t\t\tGeneric InitialModule\n\t\t\t\tGeneric startLifetime\n\t\t\t\t\tFloat scalar = 5.233333\n\t\t\t\tGeneric startSpeed\n\t\t\t\t\tFloat scalar = 6\n\t\t\tGeneric EmissionModule\n\t\t\t\tGeneric rate\n\t\t\t\t\tFloat scalar = 1\n";

    const POSITION_SERIALIZER_OVERRIDE: &str = "GameObject BackgroundObject\n\tComponent PositionSerializer\n\t\tObjectReference prefab = 4\n\t\tArray childLocalPositions\n\t\t\tArraySize size = 7\n\t\t\tElement 0\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = 62.22481\n\t\t\t\tFloat z = 50\n\t\t\tElement 5\n\t\t\t\tVector3 data\n\t\t\t\tFloat z = -5\n";

    const LIT_AREA_OVERRIDE: &str = "GameObject LitArea\n\tComponent MentalTools.BezierCurve\n\t\tInteger bezierPointCount = 272\n\t\tGeneric bezierCurve\n\t\t\tArray nodes\n\t\t\t\tArraySize size = 2\n\t\t\t\tElement 0\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = -25.48112\n\t\t\t\t\t\t\tFloat y = 4.01677\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0.03105164\n\t\t\t\t\t\t\tFloat y = 3.955859\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -0.03105164\n\t\t\t\t\t\t\tFloat y = -3.955859\n\t\t\t\tElement 1\n\t\t\t\t\tGeneric data\n\t\t\t\t\t\tVector3 position\n\t\t\t\t\t\t\tFloat x = -19.5656\n\t\t\t\t\t\t\tFloat y = 8.65094\n\t\t\t\t\t\tVector3 tangent0\n\t\t\t\t\t\t\tFloat x = 0.5396729\n\t\t\t\t\t\t\tFloat y = 0.8624678\n\t\t\t\t\t\tVector3 tangent1\n\t\t\t\t\t\t\tFloat x = -0.5396729\n\t\t\t\t\t\t\tFloat y = -0.8624678\n\tComponent MentalTools.BezierMesh\n\t\tFloat borderWidth = 0.5\n";

    #[test]
    fn parses_particle_system_generics_through_runtime_model() {
        let document = RuntimeOverrideDocument::parse(WINDAREA_OVERRIDE);
        let root = document.root("GameObject", "WindArea").expect("missing root");
        let wind_area = root.component("WindArea").expect("missing component");
        let handle = wind_area
            .child("Vector3", "windDirectionHandle")
            .and_then(|node| node.value_as_vec3())
            .expect("missing handle");
        let particle = root
            .child("GameObject", "WindEffect1")
            .and_then(|child| child.component("ParticleSystem"))
            .expect("missing particle system");

        assert!((handle.x - 17.67106).abs() < 0.0001);
        assert_eq!(
            wind_area
                .child("Float", "m_windPowerFactor")
                .and_then(|node| node.value_as_f32()),
            Some(0.26)
        );
        assert_eq!(
            particle
                .child("Generic", "InitialModule")
                .and_then(|module| module.child("Generic", "startLifetime"))
                .and_then(|field| field.child("Float", "scalar"))
                .and_then(|node| node.value_as_f32()),
            Some(5.233333)
        );
    }

    #[test]
    fn parses_position_serializer_array_payloads() {
        let document = RuntimeOverrideDocument::parse(POSITION_SERIALIZER_OVERRIDE);
        let root = document
            .root("GameObject", "BackgroundObject")
            .expect("missing root");
        let serializer = root
            .component("PositionSerializer")
            .expect("missing serializer");
        let positions = serializer
            .child("Array", "childLocalPositions")
            .and_then(|node| node.as_array())
            .expect("missing array");

        assert_eq!(positions.size, Some(7));
        assert_eq!(
            serializer
                .child("ObjectReference", "prefab")
                .and_then(|node| node.value_as_i32()),
            Some(4)
        );
        let first = positions
            .element(0)
            .and_then(|node| node.value_as_vec3())
            .expect("missing first position");
        assert!(first.x.abs() < 0.0001);
        assert!((first.y - 62.22481).abs() < 0.0001);
        assert!((first.z - 50.0).abs() < 0.0001);

        let sixth = positions
            .element(5)
            .and_then(|node| node.value_as_vec3())
            .expect("missing sixth position");
        assert!(sixth.x.abs() < 0.0001);
        assert!(sixth.y.abs() < 0.0001);
        assert!((sixth.z + 5.0).abs() < 0.0001);
    }

    #[test]
    fn parses_generic_array_elements_for_lit_area_nodes() {
        let document = RuntimeOverrideDocument::parse(LIT_AREA_OVERRIDE);
        let root = document.root("GameObject", "LitArea").expect("missing root");
        let curve = root.component("BezierCurve").expect("missing curve");
        let nodes = curve
            .child("Generic", "bezierCurve")
            .and_then(|value| value.child("Array", "nodes"))
            .and_then(|node| node.as_array())
            .expect("missing nodes");
        let first = nodes.element(0).expect("missing first node");

        assert_eq!(
            curve
                .child("Integer", "bezierPointCount")
                .and_then(|node| node.value_as_i32()),
            Some(272)
        );
        let position = first
            .child("Vector3", "position")
            .and_then(|node| node.value_as_vec3())
            .expect("missing position");
        assert!((position.x + 25.48112).abs() < 0.0001);
        assert!((position.y - 4.01677).abs() < 0.0001);
        assert!(position.z.abs() < 0.0001);
        assert_eq!(
            root.component("BezierMesh")
                .and_then(|mesh| mesh.child("Float", "borderWidth"))
                .and_then(|node| node.value_as_f32()),
            Some(0.5)
        );
    }
}