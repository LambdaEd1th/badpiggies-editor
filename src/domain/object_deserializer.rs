//! 1:1 Rust port of Unity Assembly-CSharp `ObjectDeserializer` plus the
//! `LevelLoader.ReadPrefabOverrides()` entry point that wraps it.
//!
//! Source of truth: `Assets/Scripts/Assembly-CSharp/ObjectDeserializer.cs` and
//! the relevant slice of `Assets/Scripts/Assembly-CSharp/LevelLoader.cs`.
//!
//! The original Unity code is built on C# reflection: it walks a tab-indented
//! UTF-8 stream and uses `FieldInfo` / `PropertyInfo` to assign values into
//! arbitrary `Component` instances. Rust has no equivalent runtime reflection,
//! so the reflective binding is delegated to a [`RuntimeHost`] trait that the
//! caller implements; the *state machine* that consumes the stream, the
//! per-shape dispatch tables, and every Unity-specific quirk (Camera
//! background color, BoxCollider center/size, Transform local position/
//! rotation/scale, Keyframe in/out tangent rewrite, Behaviour.m_Enabled,
//! Rigidbody isKinematic, ParticleSystem startLifetime/startSpeed, the silent
//! `m_ObjectHideFlags`-style ignore list, and so on) live here unchanged.
//!
//! The reader exactly mirrors Unity's `ObjectDeserializer.ObjectReader`,
//! including its quirks: `ReadLine` clears the cached indentation flag,
//! `GetIndentation` peeks tabs only, and `ReadProperty` splits on `' '` with
//! the special `... = value` recombination for multi-word property names.

use crate::domain::types::{Color, Vec2, Vec3};

// ===========================================================================
// Public reflection surface (Unity reflection replacement)
// ===========================================================================

/// Trait implemented by the caller to supply the Unity-side reflection that
/// `ObjectDeserializer` would normally do at runtime. Handle types are opaque
/// integers/strings the host hands back from lookups.
pub trait RuntimeHost {
    /// Handle representing a Unity `GameObject` instance.
    type GameObject: Copy + Eq;
    /// Handle representing a `Component` (or other reflective target such as
    /// the inner `Value` of a Generic field, or a `Keyframe`).
    type Target: Copy + Eq;

    // -- LevelLoader reference table -----------------------------------------

    /// `ObjectReader.GetReferencedObject(int index)` — returns whatever the
    /// host stored in its `m_references` list. May be `None`.
    fn referenced_object(&self, index: i32) -> Option<Value<Self>>;

    // -- GameObject hierarchy -------------------------------------------------

    /// `obj.transform.Find(name).gameObject`. Returns `None` if missing.
    fn find_child(&mut self, parent: Self::GameObject, name: &str) -> Option<Self::GameObject>;

    /// `obj.GetComponent(name)`. Returns `None` if absent.
    fn get_component(&mut self, obj: Self::GameObject, name: &str) -> Option<Self::Target>;

    /// `obj.AddComponent(ComponentHelper.GetComponentTypeByName(name))`. Returns
    /// `None` when the type lookup fails — matching the C# null guard.
    fn add_component(&mut self, obj: Self::GameObject, name: &str) -> Option<Self::Target>;

    /// `gameObject.name`.
    fn game_object_name(&self, obj: Self::GameObject) -> String;

    // -- Reflection ----------------------------------------------------------

    /// Mirror of `field.SetValue(obj, value)` plus the `SetProperty` quirk
    /// table at the top of `ObjectDeserializer.SetProperty`. The walker has
    /// already applied the quirks (renames, special casts, silent ignores)
    /// before calling this method.
    fn set_field(&mut self, target: Self::Target, field: &str, value: Value<Self>);

    /// Mirror of `field.GetValue(obj)` for the Generic/Color/Vector*/etc.
    /// branch: the walker reads the current value, recursively patches sub-
    /// fields, then writes the result back via [`Self::set_field`].
    fn get_field(&mut self, target: Self::Target, field: &str) -> Option<Value<Self>>;

    /// Used by `ReadGeneric` recursion when the host stores Component-like
    /// structs (Keyframe, embedded Generic values). Returns a `Target` handle
    /// that can be mutated. The default takes the existing `Value` and treats
    /// it as a Generic mapping the walker can extend in place.
    fn as_struct_target(&mut self, value: &mut Value<Self>) -> Option<Self::Target> {
        let _ = value;
        None
    }

    /// `obj.SendMessage("OnDataLoaded", SendMessageOptions.DontRequireReceiver)`.
    fn send_message(&mut self, obj: Self::GameObject, message: &str);

    // -- ReadComponent fallback hooks ---------------------------------------

    /// Hook for the `component is ParticleSystem` branch.
    fn is_particle_system(&self, target: Self::Target) -> bool {
        let _ = target;
        false
    }
    fn set_particle_start_lifetime(&mut self, target: Self::Target, value: f32) {
        let _ = (target, value);
    }
    fn set_particle_start_speed(&mut self, target: Self::Target, value: f32) {
        let _ = (target, value);
    }
    /// Extension over the strict Unity port: Unity's reader silently drops
    /// `EmissionModule.rate.scalar`, but the editor needs the value for
    /// rendering. Hosts that care can override.
    fn set_particle_emission_rate(&mut self, target: Self::Target, value: f32) {
        let _ = (target, value);
    }

    /// Extension over the strict Unity port: when the override stream cites an
    /// `ObjectReference` index that `referenced_object` can't resolve, hosts
    /// can stash the raw index here (Unity's loader drops it). Default no-op.
    fn set_object_reference_index(
        &mut self,
        target: Self::Target,
        field: &str,
        index: i32,
    ) {
        let _ = (target, field, index);
    }

    // -- AnimationCurve / Array hooks ---------------------------------------

    /// `ReadAnimationCurve` collects all keys into a flat list, then hands it
    /// to the host (Unity uses reflection to rebuild the `keys` array on the
    /// curve object). Hosts that don't care can ignore this.
    fn set_animation_curve(
        &mut self,
        target: Self::Target,
        field: &str,
        keys: Vec<Keyframe>,
    ) {
        let _ = (target, field, keys);
    }

    /// `ReadArray` collects the parsed array into an ordered `(index, value)`
    /// list (size + sparse element insertions, matching Unity's IList semantics
    /// where missing indices are filled with default values). Hosts apply it
    /// however they store collections.
    fn set_array(
        &mut self,
        target: Self::Target,
        field: &str,
        size: i32,
        elements: Vec<ArrayElement<Self>>,
    ) {
        let _ = (target, field, size, elements);
    }
}

/// Value handed across the reflection boundary. Mirrors the variants
/// `ObjectDeserializer` materializes in C# (`int`, `float`, `string`, `bool`,
/// `Bounds`, `Color`, `Vector2`, `Vector3`, `Quaternion`, `Rect`, `Keyframe`,
/// `UnityEngine.Object` references and Generic recursion).
pub enum Value<H: RuntimeHost + ?Sized> {
    Null,
    Integer(i32),
    Float(f32),
    String(String),
    Boolean(bool),
    Enum(i32),
    Bounds(Bounds),
    Color(Color),
    Vector2(Vec2),
    Vector3(Vec3),
    Quaternion(Quaternion),
    Rect(Rect),
    Keyframe(Keyframe),
    ObjectReference(H::GameObject),
    /// Generic struct: walker reads sub-fields into this `(name, Value)`
    /// vector. Hosts that already store the struct as a typed object usually
    /// override [`RuntimeHost::as_struct_target`] and let the walker write
    /// directly into their object instead of accumulating into this variant.
    Generic(Vec<(String, Value<H>)>),
}

impl<H: RuntimeHost + ?Sized> std::fmt::Debug for Value<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Null => write!(f, "Null"),
            Value::Integer(v) => f.debug_tuple("Integer").field(v).finish(),
            Value::Float(v) => f.debug_tuple("Float").field(v).finish(),
            Value::String(v) => f.debug_tuple("String").field(v).finish(),
            Value::Boolean(v) => f.debug_tuple("Boolean").field(v).finish(),
            Value::Enum(v) => f.debug_tuple("Enum").field(v).finish(),
            Value::Bounds(v) => f.debug_tuple("Bounds").field(v).finish(),
            Value::Color(v) => f.debug_tuple("Color").field(v).finish(),
            Value::Vector2(v) => f.debug_tuple("Vector2").field(v).finish(),
            Value::Vector3(v) => f.debug_tuple("Vector3").field(v).finish(),
            Value::Quaternion(v) => f.debug_tuple("Quaternion").field(v).finish(),
            Value::Rect(v) => f.debug_tuple("Rect").field(v).finish(),
            Value::Keyframe(v) => f.debug_tuple("Keyframe").field(v).finish(),
            Value::ObjectReference(_) => write!(f, "ObjectReference(..)"),
            Value::Generic(entries) => f.debug_tuple("Generic").field(entries).finish(),
        }
    }
}

impl<H: RuntimeHost + ?Sized> Clone for Value<H> {
    fn clone(&self) -> Self {
        match self {
            Value::Null => Value::Null,
            Value::Integer(v) => Value::Integer(*v),
            Value::Float(v) => Value::Float(*v),
            Value::String(v) => Value::String(v.clone()),
            Value::Boolean(v) => Value::Boolean(*v),
            Value::Enum(v) => Value::Enum(*v),
            Value::Bounds(v) => Value::Bounds(*v),
            Value::Color(v) => Value::Color(*v),
            Value::Vector2(v) => Value::Vector2(*v),
            Value::Vector3(v) => Value::Vector3(*v),
            Value::Quaternion(v) => Value::Quaternion(*v),
            Value::Rect(v) => Value::Rect(*v),
            Value::Keyframe(v) => Value::Keyframe(*v),
            Value::ObjectReference(v) => Value::ObjectReference(*v),
            Value::Generic(entries) => Value::Generic(entries.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub struct Bounds {
    pub center: Vec3,
    pub extents: Vec3,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Keyframe {
    pub time: f32,
    pub value: f32,
    pub in_tangent: f32,
    pub out_tangent: f32,
}

/// One parsed entry from `ReadArray`. `value` is whatever `ReadValueType` /
/// reference / Generic recursion produced for the element.
pub struct ArrayElement<H: RuntimeHost + ?Sized> {
    pub index: i32,
    pub value: Value<H>,
}

impl<H: RuntimeHost + ?Sized> Clone for ArrayElement<H> {
    fn clone(&self) -> Self {
        Self { index: self.index, value: self.value.clone() }
    }
}

impl<H: RuntimeHost + ?Sized> std::fmt::Debug for ArrayElement<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ArrayElement")
            .field("index", &self.index)
            .field("value", &self.value)
            .finish()
    }
}

// ===========================================================================
// PropertyData + ObjectReader  (verbatim Unity port)
// ===========================================================================

/// Mirror of `ObjectDeserializer.PropertyData`.
#[derive(Debug, Clone, Default)]
pub struct PropertyData {
    pub r#type: String,
    pub name: String,
    pub value: String,
}

impl PropertyData {
    fn typed(r#type: String, name: String) -> Self {
        Self {
            r#type,
            name,
            value: String::new(),
        }
    }

    fn valued(r#type: String, name: String, value: String) -> Self {
        Self {
            r#type,
            name,
            value,
        }
    }

    /// `int.Parse(this.value)`.
    pub fn integer_value(&self) -> i32 {
        self.value.parse::<i32>().unwrap_or(0)
    }

    /// `float.Parse(this.value, CultureInfo.InvariantCulture)`.
    pub fn float_value(&self) -> f32 {
        self.value.parse::<f32>().unwrap_or(0.0)
    }

    /// `value.Substring(1, value.Length - 2)` — Unity strips one character off
    /// each end (the surrounding quote characters as written by its exporter).
    pub fn string_value(&self) -> String {
        let len = self.value.chars().count();
        if len >= 2 {
            self.value.chars().skip(1).take(len - 2).collect()
        } else {
            String::new()
        }
    }

    /// `value == "True"`.
    pub fn bool_value(&self) -> bool {
        self.value == "True"
    }
}

/// Mirror of `ObjectDeserializer.ObjectReader`. Reads from a UTF-8 string in
/// the same way Unity reads from a `StreamReader`: line by line, with a
/// `Peek`/`Read`-style indentation probe that consumes leading tabs.
pub struct ObjectReader {
    chars: Vec<char>,
    cursor: usize,
    indentation: i32,
    indentation_read: bool,
}

impl ObjectReader {
    pub fn new(text: &str) -> Self {
        Self {
            chars: text.chars().collect(),
            cursor: 0,
            indentation: 0,
            indentation_read: false,
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.cursor).copied()
    }

    fn read_char(&mut self) -> Option<char> {
        let c = self.peek()?;
        self.cursor += 1;
        Some(c)
    }

    /// `private string ReadLine()` — resets the cached indentation flag and
    /// reads up to the next `\n` (Unity's `StreamReader.ReadLine` handles
    /// either CR, LF or CRLF as a line terminator).
    fn read_line(&mut self) -> Option<String> {
        self.indentation_read = false;
        if self.cursor >= self.chars.len() {
            return None;
        }
        let mut out = String::new();
        while let Some(c) = self.peek() {
            if c == '\n' {
                self.cursor += 1;
                break;
            }
            if c == '\r' {
                self.cursor += 1;
                if self.peek() == Some('\n') {
                    self.cursor += 1;
                }
                break;
            }
            out.push(c);
            self.cursor += 1;
        }
        Some(out)
    }

    /// `public PropertyData ReadProperty()` — splits on space, with the special
    /// `... = value` recombination Unity uses for multi-word property names.
    pub fn read_property(&mut self) -> Option<PropertyData> {
        let line = self.read_line()?;
        let parts: Vec<&str> = line.split(' ').collect();
        if parts.len() == 2 {
            return Some(PropertyData::typed(parts[0].to_string(), parts[1].to_string()));
        }
        if parts.len() == 4 && parts[2] == "=" {
            return Some(PropertyData::valued(
                parts[0].to_string(),
                parts[1].to_string(),
                parts[3].to_string(),
            ));
        }
        if parts.len() > 4 {
            let mut combined_name = String::new();
            for i in 1..parts.len() {
                if parts[i] == "=" {
                    break;
                }
                if i > 1 {
                    combined_name.push(' ');
                }
                combined_name.push_str(parts[i]);
            }
            return Some(PropertyData::valued(
                parts[0].to_string(),
                combined_name,
                parts[parts.len() - 1].to_string(),
            ));
        }
        None
    }

    /// `public PropertyData ReadTypeAndName()` — splits on space exactly once.
    pub fn read_type_and_name(&mut self) -> Option<PropertyData> {
        let line = self.read_line()?;
        let mut iter = line.splitn(2, ' ');
        let r#type = iter.next()?;
        let name = iter.next()?;
        Some(PropertyData::typed(r#type.to_string(), name.to_string()))
    }

    /// `public int GetIndentation()` — caches the tab count until the next
    /// `ReadLine`.
    pub fn get_indentation(&mut self) -> i32 {
        if self.indentation_read {
            return self.indentation;
        }
        let mut count = 0;
        while self.peek() == Some('\t') {
            count += 1;
            self.read_char();
        }
        self.indentation = count;
        self.indentation_read = true;
        count
    }
}

// ===========================================================================
// LevelLoader.ReadPrefabOverrides — 1:1 wrapper
// ===========================================================================

/// `private void ReadPrefabOverrides(GameObject obj, BinaryReader reader)`.
///
/// In Unity this peels the 4-byte length-prefixed UTF-8 payload off the level
/// binary, wraps it in a `StreamReader`/`ObjectReader`, calls
/// `ObjectDeserializer.ReadFile`, then does
/// `obj.SendMessage("OnDataLoaded", SendMessageOptions.DontRequireReceiver)`.
///
/// In Rust the level binary is already split into [`PrefabOverrideData`] up
/// the stack (see `domain::types::PrefabOverrideData`), so this function takes
/// the already-decoded UTF-8 text directly.
///
/// [`PrefabOverrideData`]: crate::domain::types::PrefabOverrideData
pub fn read_prefab_overrides<H: RuntimeHost>(host: &mut H, obj: H::GameObject, text: &str) {
    let mut reader = ObjectReader::new(text);
    read_file(host, obj, &mut reader);
    host.send_message(obj, "OnDataLoaded");
}

// ===========================================================================
// ObjectDeserializer — 1:1 port of the C# static methods
// ===========================================================================

/// `public static void ReadFile(GameObject obj, ObjectReader reader)`.
pub fn read_file<H: RuntimeHost>(host: &mut H, obj: H::GameObject, reader: &mut ObjectReader) {
    let Some(property) = reader.read_type_and_name() else {
        return;
    };
    if property.r#type == "GameObject" && property.name == host.game_object_name(obj) {
        read_object(host, obj, 1, reader);
    }
}

/// `private static void ReadObject(GameObject obj, int depth, ObjectReader reader)`.
fn read_object<H: RuntimeHost>(
    host: &mut H,
    obj: H::GameObject,
    depth: i32,
    reader: &mut ObjectReader,
) {
    while reader.get_indentation() == depth {
        let Some(property) = reader.read_type_and_name() else {
            return;
        };
        if property.r#type == "Component" {
            let short_name: &str = if let Some(idx) = property.name.rfind('.') {
                &property.name[idx + 1..]
            } else {
                property.name.as_str()
            };
            let mut component = host.get_component(obj, short_name);
            if component.is_none() {
                component = host.add_component(obj, short_name);
            }
            if let Some(target) = component {
                if host.is_particle_system(target) {
                    read_particle_system(host, target, depth + 1, reader);
                } else {
                    read_component(host, target, depth + 1, reader);
                }
            } else {
                // C# leaves a stranded sub-tree alone (no walker to consume it),
                // which leaves the reader pointer stuck on its children. Mirror
                // that by silently draining the orphan children.
                drain_children(reader, depth + 1);
            }
        } else if property.r#type == "GameObject" {
            if let Some(child) = host.find_child(obj, &property.name) {
                read_object(host, child, depth + 1, reader);
            } else {
                drain_children(reader, depth + 1);
            }
        }
    }
}

fn drain_children(reader: &mut ObjectReader, depth: i32) {
    while reader.get_indentation() >= depth {
        if reader.read_line().is_none() {
            break;
        }
    }
}

/// `private static void ReadComponent(object component, int depth, ObjectReader reader)`.
fn read_component<H: RuntimeHost>(
    host: &mut H,
    component: H::Target,
    depth: i32,
    reader: &mut ObjectReader,
) {
    while reader.get_indentation() == depth {
        let Some(property) = reader.read_property() else {
            return;
        };
        match property.r#type.as_str() {
            "Integer" => set_property(
                host,
                component,
                &property.name,
                Value::Integer(property.integer_value()),
            ),
            "Float" => set_property(
                host,
                component,
                &property.name,
                Value::Float(property.float_value()),
            ),
            "String" => {
                set_property(
                    host,
                    component,
                    &property.name,
                    Value::String(property.string_value()),
                );
                // Unity unconditionally consumes one extra property line after
                // a string: `reader.ReadProperty();`. Preserve that.
                let _ = reader.read_property();
            }
            "Boolean" => set_property(
                host,
                component,
                &property.name,
                Value::Boolean(property.bool_value()),
            ),
            "Enum" => set_property(
                host,
                component,
                &property.name,
                Value::Enum(property.integer_value()),
            ),
            "Bounds" => {
                let bounds = read_bounds(reader, depth + 1);
                set_property(host, component, &property.name, Value::Bounds(bounds));
            }
            "ObjectReference" => {
                let resolved = host.referenced_object(property.integer_value());
                if let Some(value) = resolved {
                    set_property(host, component, &property.name, value);
                } else {
                    host.set_object_reference_index(
                        component,
                        &property.name,
                        property.integer_value(),
                    );
                }
                // Unity reads up to two follow-up properties at depth+1 (the
                // serializer used to emit additional `m_FileID`/`m_PathID`
                // tags after each reference).
                if reader.get_indentation() == depth + 1 {
                    let _ = reader.read_property();
                }
                if reader.get_indentation() == depth + 1 {
                    let _ = reader.read_property();
                }
            }
            "Array" => {
                read_array(host, component, &property.name, depth + 1, reader);
            }
            "AnimationCurve" => {
                read_animation_curve(host, component, &property.name, depth + 1, reader);
            }
            "Generic" | "Color" | "Vector2" | "Vector3" | "Rect" | "16" | "Quaternion" => {
                // Read-modify-write through the host.
                let current = host
                    .get_field(component, &property.name)
                    .unwrap_or_else(|| default_for_type(&property.r#type));
                let mut working = current;
                read_generic(host, &mut working, depth + 1, reader);
                set_property(host, component, &property.name, working);
            }
            _ => {
                // Unknown shape — drain its children to keep the reader aligned.
                drain_children(reader, depth + 1);
            }
        }
    }
}

/// `private static void SetProperty(object obj, string name, object value)`.
///
/// All of Unity's quirk renames live here in one table.
fn set_property<H: RuntimeHost>(
    host: &mut H,
    target: H::Target,
    name: &str,
    value: Value<H>,
) {
    // Silent ignore list — these names hit the final `else if` and Unity does
    // nothing with them. Reproduce that here so the host never sees them.
    match name {
        "m_ObjectHideFlags"
        | "m_EditorHideFlags"
        | "m_Name"
        | "m_PrefabParentObject"
        | "m_PrefabInternal"
        | "m_GameObject"
        | "m_EditorClassIdentifier"
        | "m_Script"
        | "m_Mesh"
        | "m_ConnectedBody"
        | "m_RootOrder"
        | "m_Mass" => return,
        _ => {}
    }

    // Field-/property-rename quirks. These mirror the C# branches one-for-one.
    let resolved_name: &str = match name {
        // Keyframe.time -> m_Time. Same logic in C# uses `obj is Keyframe`; in
        // our port the host distinguishes the target type — we just hand both
        // the original and renamed names off via the same set_field, and the
        // host can disambiguate. We pre-rename here so hosts don't have to.
        "time" => "m_Time",
        "inSlope" => "m_InTangent",
        "outSlope" => "m_OutTangent",
        // Rigidbody.m_IsKinematic -> isKinematic.
        "m_IsKinematic" => "isKinematic",
        // Behaviour.m_Enabled -> enabled (handled by host; pass name through).
        // Camera.orthographic size -> orthographicSize.
        "orthographic size" => "orthographicSize",
        // Behaviour.m_Enabled stays as m_Enabled and the host is expected to
        // route it to `enabled`; we don't rename it here because the C# code
        // also falls through to a `(Behaviour)obj).enabled = ...` branch
        // *only* if reflection failed. Hosts get the original name.
        other => other,
    };

    host.set_field(target, resolved_name, value);
}

/// `private static Bounds ReadBounds(int depth, ObjectReader reader)`.
fn read_bounds(reader: &mut ObjectReader, depth: i32) -> Bounds {
    let mut bounds = Bounds::default();
    let _ = reader.get_indentation();
    let _ = reader.read_property();
    bounds.center = read_vector3(reader, depth + 1);
    let _ = reader.get_indentation();
    let _ = reader.read_property();
    bounds.extents = read_vector3(reader, depth + 1);
    bounds
}

/// `private static Vector3 ReadVector3(int depth, ObjectReader reader)`.
fn read_vector3(reader: &mut ObjectReader, depth: i32) -> Vec3 {
    let mut v = Vec3::default();
    for _ in 0..3 {
        if reader.get_indentation() == depth {
            let Some(prop) = reader.read_property() else {
                continue;
            };
            match prop.name.as_str() {
                "x" => v.x = prop.float_value(),
                "y" => v.y = prop.float_value(),
                "z" => v.z = prop.float_value(),
                _ => {}
            }
        }
    }
    v
}

/// `private static object GetDefaultValue(Type type)` plus the upstream
/// `ReadValueType` shape probing.
fn default_for_type<H: RuntimeHost>(type_name: &str) -> Value<H> {
    match type_name {
        "Integer" => Value::Integer(0),
        "Float" => Value::Float(0.0),
        "String" => Value::String(String::new()),
        "Boolean" => Value::Boolean(false),
        "Enum" => Value::Enum(0),
        "Bounds" => Value::Bounds(Bounds::default()),
        "Color" => Value::Color(Color::default()),
        "Vector2" => Value::Vector2(Vec2::default()),
        "Vector3" | "16" => Value::Vector3(Vec3::default()),
        "Quaternion" => Value::Quaternion(Quaternion::default()),
        "Rect" => Value::Rect(Rect::default()),
        "Keyframe" => Value::Keyframe(Keyframe::default()),
        _ => Value::Generic(Vec::new()),
    }
}

/// `private static void ReadGeneric(object obj, int depth, ObjectReader reader)`.
///
/// The C# code recursively calls `ReadComponent`, which means Unity treats the
/// inner `Value` as if it were a Component — using reflection to set fields
/// like `x` / `y` / `z` on the Vector3 struct. Our port writes through the
/// `Value` enum in place: each sub-property either lands in a known scalar
/// slot (x/y/z/w/r/g/b/a/...) or, for `Generic(Vec<(name, Value)>)`, gets
/// appended.
fn read_generic<H: RuntimeHost>(
    _host: &mut H,
    value: &mut Value<H>,
    depth: i32,
    reader: &mut ObjectReader,
) {
    while reader.get_indentation() == depth {
        let Some(property) = reader.read_property() else {
            return;
        };
        match property.r#type.as_str() {
            "Integer" => assign_named_integer(value, &property.name, property.integer_value()),
            "Float" => assign_named_scalar(value, &property.name, property.float_value()),
            "Enum" => assign_named_enum(value, &property.name, property.integer_value()),
            "Boolean" => assign_named_bool(value, &property.name, property.bool_value()),
            "String" => {
                assign_named_string(value, &property.name, property.string_value());
                let _ = reader.read_property();
            }
            "Vector2" | "Vector3" | "Quaternion" | "Color" | "Rect" | "Bounds" | "Generic"
            | "Keyframe" | "16" => {
                let mut nested = default_for_type::<H>(&property.r#type);
                read_generic(_host, &mut nested, depth + 1, reader);
                assign_named_struct(value, &property.name, nested);
            }
            "Array" => {
                // Nested array inside a Generic struct: produce a synthetic
                // `Value::Generic` with the same shape `set_array` emits
                // (size + sparse indexed elements). Lets typed components
                // pattern-match against arrays embedded in struct fields
                // such as `Generic bezierCurve { Array nodes { ... } }`.
                let mut nested = Value::Generic(Vec::new());
                read_array_into_value(_host, &mut nested, depth + 1, reader);
                assign_named_struct(value, &property.name, nested);
            }
            _ => {
                drain_children(reader, depth + 1);
            }
        }
    }
}

fn assign_named_integer<H: RuntimeHost + ?Sized>(value: &mut Value<H>, name: &str, integer: i32) {
    match value {
        Value::Integer(slot) if name == "scalar" => *slot = integer,
        Value::Float(slot) if name == "scalar" => *slot = integer as f32,
        Value::Generic(entries) => entries.push((name.to_string(), Value::Integer(integer))),
        _ => assign_named_scalar(value, name, integer as f32),
    }
}

fn assign_named_enum<H: RuntimeHost + ?Sized>(value: &mut Value<H>, name: &str, integer: i32) {
    match value {
        Value::Enum(slot) => *slot = integer,
        Value::Generic(entries) => entries.push((name.to_string(), Value::Enum(integer))),
        _ => assign_named_integer(value, name, integer),
    }
}

fn assign_named_scalar<H: RuntimeHost + ?Sized>(value: &mut Value<H>, name: &str, scalar: f32) {
    match value {
        Value::Vector2(v) => match name {
            "x" => v.x = scalar,
            "y" => v.y = scalar,
            _ => {}
        },
        Value::Vector3(v) => match name {
            "x" => v.x = scalar,
            "y" => v.y = scalar,
            "z" => v.z = scalar,
            _ => {}
        },
        Value::Quaternion(q) => match name {
            "x" => q.x = scalar,
            "y" => q.y = scalar,
            "z" => q.z = scalar,
            "w" => q.w = scalar,
            _ => {}
        },
        Value::Color(c) => match name {
            "r" => c.r = scalar,
            "g" => c.g = scalar,
            "b" => c.b = scalar,
            "a" => c.a = scalar,
            _ => {}
        },
        Value::Rect(r) => match name {
            "x" => r.x = scalar,
            "y" => r.y = scalar,
            "width" => r.width = scalar,
            "height" => r.height = scalar,
            _ => {}
        },
        Value::Keyframe(k) => match name {
            "time" | "m_Time" => k.time = scalar,
            "value" | "m_Value" => k.value = scalar,
            "inSlope" | "m_InTangent" => k.in_tangent = scalar,
            "outSlope" | "m_OutTangent" => k.out_tangent = scalar,
            _ => {}
        },
        Value::Float(f) if name == "scalar" => *f = scalar,
        Value::Integer(i) if name == "scalar" => *i = scalar as i32,
        Value::Generic(entries) => entries.push((name.to_string(), Value::Float(scalar))),
        _ => {}
    }
}

fn assign_named_bool<H: RuntimeHost + ?Sized>(value: &mut Value<H>, name: &str, b: bool) {
    if let Value::Generic(entries) = value {
        entries.push((name.to_string(), Value::Boolean(b)));
    } else if let Value::Boolean(slot) = value {
        let _ = name;
        *slot = b;
    }
}

fn assign_named_string<H: RuntimeHost + ?Sized>(value: &mut Value<H>, name: &str, s: String) {
    if let Value::Generic(entries) = value {
        entries.push((name.to_string(), Value::String(s)));
    } else if let Value::String(slot) = value {
        let _ = name;
        *slot = s;
    }
}

fn assign_named_struct<H: RuntimeHost + ?Sized>(
    value: &mut Value<H>,
    name: &str,
    nested: Value<H>,
) {
    match value {
        Value::Bounds(bounds) => match name {
            "m_Center" | "center" => {
                if let Value::Vector3(v) = nested {
                    bounds.center = v;
                }
            }
            "m_Extent" | "extents" => {
                if let Value::Vector3(v) = nested {
                    bounds.extents = v;
                }
            }
            _ => {}
        },
        Value::Generic(entries) => entries.push((name.to_string(), nested)),
        _ => {}
    }
}

/// `private static void ReadAnimationCurve(object component, string fieldName, int depth, ObjectReader reader)`.
fn read_animation_curve<H: RuntimeHost>(
    host: &mut H,
    component: H::Target,
    field_name: &str,
    depth: i32,
    reader: &mut ObjectReader,
) {
    let mut depth = depth;
    while reader.get_indentation() == depth {
        let _wrap_prop = reader.read_property(); // the `Generic keys` wrapper
        depth += 1;

        // Optional `Integer length = N` at depth+1
        let mut length: i32 = 0;
        if reader.get_indentation() == depth {
            if let Some(len_prop) = reader.read_property() {
                length = len_prop.integer_value();
            }
        }

        // Elements
        let mut keys: Vec<Keyframe> = Vec::with_capacity(length.max(0) as usize);
        while reader.get_indentation() == depth {
            let Some(prop) = reader.read_property() else {
                break;
            };
            if prop.r#type == "Element" {
                let element_index: i32 = prop.name.parse::<i32>().unwrap_or(0);
                while keys.len() as i32 <= element_index {
                    keys.push(Keyframe::default());
                }
                while reader.get_indentation() == depth + 1 {
                    let Some(inner) = reader.read_property() else {
                        break;
                    };
                    if inner.r#type == "Generic" || inner.r#type == "Keyframe" {
                        let mut value: Value<H> = Value::Keyframe(keys[element_index as usize]);
                        read_generic(host, &mut value, depth + 1, reader);
                        if let Value::Keyframe(k) = value {
                            keys[element_index as usize] = k;
                        }
                    }
                }
            }
        }

        depth -= 1;
        host.set_animation_curve(component, field_name, keys);
    }
}

/// `private static void ReadArray(object component, string fieldName, int depth, ObjectReader reader)`.
fn read_array<H: RuntimeHost>(
    host: &mut H,
    component: H::Target,
    field_name: &str,
    depth: i32,
    reader: &mut ObjectReader,
) {
    // `ArraySize size = N` at depth
    let mut size: i32 = 0;
    if reader.get_indentation() == depth {
        if let Some(prop) = reader.read_property() {
            size = prop.integer_value();
        }
    }

    let mut elements: Vec<ArrayElement<H>> = Vec::new();
    while reader.get_indentation() == depth {
        let Some(prop) = reader.read_property() else {
            break;
        };
        if prop.r#type == "Element" {
            let element_index: i32 = prop.name.parse::<i32>().unwrap_or(0);

            // Peek the first sub-property to decide value-type vs reference vs
            // generic, mirroring Unity's `type.IsValueType` /
            // `type.IsSubclassOf(typeof(UnityEngine.Object))` / else branches.
            // Unity disambiguates via reflection on the IList<T> element type;
            // we disambiguate by looking at what the stream emits next.
            if reader.get_indentation() != depth + 1 {
                continue;
            }
            // Take a look at the very next property without losing it.
            let inner = match reader.read_property() {
                Some(p) => p,
                None => break,
            };
            match inner.r#type.as_str() {
                "Integer" | "Float" | "Boolean" | "Enum" => {
                    // Direct scalar value-type element.
                    let value = match inner.r#type.as_str() {
                        "Integer" => Value::Integer(inner.integer_value()),
                        "Float" => Value::Float(inner.float_value()),
                        "Boolean" => Value::Boolean(inner.bool_value()),
                        _ => Value::Enum(inner.integer_value()),
                    };
                    elements.push(ArrayElement { index: element_index, value });
                }
                "String" => {
                    let value = Value::String(inner.string_value());
                    let _ = reader.read_property();
                    elements.push(ArrayElement { index: element_index, value });
                }
                "ObjectReference" => {
                    let resolved = host.referenced_object(inner.integer_value());
                    if let Some(value) = resolved {
                        elements.push(ArrayElement { index: element_index, value });
                    } else {
                        elements.push(ArrayElement { index: element_index, value: Value::Null });
                    }
                }
                "Vector2" | "Vector3" | "Quaternion" | "Color" | "Rect" | "Bounds"
                | "Keyframe" | "16" => {
                    let mut value = default_for_type::<H>(&inner.r#type);
                    read_generic(host, &mut value, depth + 2, reader);
                    elements.push(ArrayElement { index: element_index, value });
                }
                "Generic" => {
                    // Generic element: walk children into a Value::Generic.
                    let mut value: Value<H> = Value::Generic(Vec::new());
                    let child_depth = reader.get_indentation();
                    if child_depth > depth {
                        read_generic(host, &mut value, child_depth, reader);
                    }
                    elements.push(ArrayElement { index: element_index, value });
                }
                _ => {
                    drain_children(reader, depth + 2);
                }
            }

            // Unity's reader sometimes leaves a stray sibling `Float`/`Vector*`
            // line after a `Vector3 data` element (see runtime model notes in
            // `prefab_override_runtime.rs`). Roll those into the same element.
            while reader.get_indentation() == depth + 1 {
                let Some(extra) = reader.read_property() else {
                    break;
                };
                if extra.r#type == "Float" {
                    if let Some(last) = elements.last_mut() {
                        assign_named_scalar(&mut last.value, &extra.name, extra.float_value());
                    }
                } else {
                    // Anything else is a new logical element; rewind by
                    // re-applying it through the same logic.
                    if extra.r#type == "Element" {
                        // Not expected mid-element, but skip safely.
                        drain_children(reader, depth + 2);
                    } else {
                        drain_children(reader, depth + 2);
                    }
                    break;
                }
            }
        }
    }

    host.set_array(component, field_name, size, elements);
}

/// Variant of [`read_array`] that writes the parsed array into an in-memory
/// `Value::Generic` instead of calling `host.set_array`. Used when an Array
/// is encountered nested inside a Generic struct field (e.g.
/// `Generic bezierCurve { Array nodes { ... } }`), where there's no target
/// `H::Target` for `set_array` to write into.
///
/// Mirrors the serialization shape that the `Scene` host's `set_array` adapter
/// emits: an entry list of `("size", Integer(N))` followed by one
/// `("<index>", value)` per element, so consumers see the same structure
/// regardless of whether the array was top-level or nested.
fn read_array_into_value<H: RuntimeHost>(
    host: &mut H,
    out: &mut Value<H>,
    depth: i32,
    reader: &mut ObjectReader,
) {
    let mut size: i32 = 0;
    if reader.get_indentation() == depth {
        if let Some(prop) = reader.read_property() {
            size = prop.integer_value();
        }
    }

    let Value::Generic(entries) = out else {
        return;
    };
    entries.push(("size".to_string(), Value::Integer(size)));

    while reader.get_indentation() == depth {
        let Some(prop) = reader.read_property() else {
            break;
        };
        if prop.r#type != "Element" {
            continue;
        }
        let element_index: i32 = prop.name.parse::<i32>().unwrap_or(0);
        if reader.get_indentation() != depth + 1 {
            continue;
        }
        let inner = match reader.read_property() {
            Some(p) => p,
            None => break,
        };
        let element_value: Value<H> = match inner.r#type.as_str() {
            "Integer" => Value::Integer(inner.integer_value()),
            "Float" => Value::Float(inner.float_value()),
            "Boolean" => Value::Boolean(inner.bool_value()),
            "Enum" => Value::Enum(inner.integer_value()),
            "String" => {
                let v = Value::String(inner.string_value());
                let _ = reader.read_property();
                v
            }
            "ObjectReference" => host
                .referenced_object(inner.integer_value())
                .unwrap_or(Value::Null),
            "Vector2" | "Vector3" | "Quaternion" | "Color" | "Rect" | "Bounds" | "Keyframe"
            | "16" => {
                let mut value = default_for_type::<H>(&inner.r#type);
                read_generic(host, &mut value, depth + 2, reader);
                value
            }
            "Generic" => {
                let mut value: Value<H> = Value::Generic(Vec::new());
                let child_depth = reader.get_indentation();
                if child_depth > depth {
                    read_generic(host, &mut value, child_depth, reader);
                }
                value
            }
            _ => {
                drain_children(reader, depth + 2);
                continue;
            }
        };
        entries.push((element_index.to_string(), element_value));

        while reader.get_indentation() == depth + 1 {
            let Some(extra) = reader.read_property() else {
                break;
            };
            if extra.r#type == "Float" {
                if let Some((_, last)) = entries.last_mut() {
                    assign_named_scalar(last, &extra.name, extra.float_value());
                }
            } else {
                drain_children(reader, depth + 2);
                break;
            }
        }
    }
}

/// `private static void ReadParticleSystemModule(ParticleSystem particleSystem, string module, int depth, ObjectReader reader)`.
fn read_particle_system_module<H: RuntimeHost>(
    host: &mut H,
    particle_system: H::Target,
    module: &str,
    depth: i32,
    reader: &mut ObjectReader,
) {
    while reader.get_indentation() == depth {
        let Some(property) = reader.read_property() else {
            return;
        };
        match module {
            "InitialModule" => {
                if property.r#type == "Generic" && property.name == "startLifetime" {
                    if reader.get_indentation() == depth + 1 {
                        if let Some(inner) = reader.read_property() {
                            host.set_particle_start_lifetime(particle_system, inner.float_value());
                        }
                    }
                } else if property.r#type == "Generic"
                    && property.name == "startSpeed"
                    && reader.get_indentation() == depth + 1
                {
                    if let Some(inner) = reader.read_property() {
                        host.set_particle_start_speed(particle_system, inner.float_value());
                    }
                }
            }
            "EmissionModule" => {
                if property.r#type == "Generic"
                    && property.name == "rate"
                    && reader.get_indentation() == depth + 1
                {
                    if let Some(inner) = reader.read_property() {
                        host.set_particle_emission_rate(particle_system, inner.float_value());
                    }
                }
            }
            "ShapeModule" => {
                // C# body is `if (!(... enabled == true)) {}` — i.e. nothing.
                let _ = property;
            }
            _ => {}
        }
    }
}

/// `private static void ReadParticleSystem(ParticleSystem particleSystem, int depth, ObjectReader reader)`.
fn read_particle_system<H: RuntimeHost>(
    host: &mut H,
    particle_system: H::Target,
    depth: i32,
    reader: &mut ObjectReader,
) {
    while reader.get_indentation() == depth {
        let Some(property) = reader.read_property() else {
            return;
        };
        if property.r#type == "Generic"
            && matches!(
                property.name.as_str(),
                "InitialModule" | "EmissionModule" | "ShapeModule"
            )
        {
            let module = property.name.clone();
            read_particle_system_module(host, particle_system, &module, depth + 1, reader);
        }
    }
}

// ===========================================================================
// Tests — exercise the state machine with a mock host on real override text
// ===========================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// A minimal in-memory Unity-ish scene: GameObjects are indexed by id, each
    /// has a name + child list + named components. Components store fields in
    /// a (name -> Value) map. Send-message calls and particle-system mutations
    /// are recorded for assertions.
    #[derive(Default)]
    struct MockScene {
        next_object: u32,
        next_target: u32,
        objects: HashMap<u32, MockObject>,
        targets: HashMap<u32, MockTarget>,
        messages: Vec<(u32, String)>,
    }

    #[derive(Default)]
    struct MockObject {
        name: String,
        children: Vec<u32>,
        components: HashMap<String, u32>,
    }

    #[derive(Default)]
    struct MockTarget {
        kind: String,
        fields: HashMap<String, Value<MockScene>>,
        arrays: HashMap<String, (i32, Vec<ArrayElement<MockScene>>)>,
        curves: HashMap<String, Vec<Keyframe>>,
        particle_lifetime: Option<f32>,
        particle_speed: Option<f32>,
    }

    impl MockScene {
        fn new() -> Self {
            Self::default()
        }
        fn add_object(&mut self, name: &str) -> u32 {
            self.next_object += 1;
            let id = self.next_object;
            self.objects.insert(
                id,
                MockObject {
                    name: name.to_string(),
                    children: Vec::new(),
                    components: HashMap::new(),
                },
            );
            id
        }
        fn add_child(&mut self, parent: u32, name: &str) -> u32 {
            let id = self.add_object(name);
            self.objects.get_mut(&parent).unwrap().children.push(id);
            id
        }
        fn add_target(&mut self, obj: u32, kind: &str) -> u32 {
            self.next_target += 1;
            let id = self.next_target;
            self.targets.insert(
                id,
                MockTarget {
                    kind: kind.to_string(),
                    ..Default::default()
                },
            );
            self.objects
                .get_mut(&obj)
                .unwrap()
                .components
                .insert(kind.to_string(), id);
            id
        }
    }

    impl RuntimeHost for MockScene {
        type GameObject = u32;
        type Target = u32;

        fn referenced_object(&self, _index: i32) -> Option<Value<Self>> {
            None
        }

        fn find_child(&mut self, parent: u32, name: &str) -> Option<u32> {
            let children = self.objects.get(&parent)?.children.clone();
            for id in children {
                if self.objects.get(&id)?.name == name {
                    return Some(id);
                }
            }
            None
        }

        fn get_component(&mut self, obj: u32, name: &str) -> Option<u32> {
            self.objects.get(&obj)?.components.get(name).copied()
        }

        fn add_component(&mut self, obj: u32, name: &str) -> Option<u32> {
            Some(self.add_target(obj, name))
        }

        fn game_object_name(&self, obj: u32) -> String {
            self.objects
                .get(&obj)
                .map(|o| o.name.clone())
                .unwrap_or_default()
        }

        fn set_field(&mut self, target: u32, field: &str, value: Value<Self>) {
            self.targets
                .get_mut(&target)
                .unwrap()
                .fields
                .insert(field.to_string(), value);
        }

        fn get_field(&mut self, target: u32, field: &str) -> Option<Value<Self>> {
            self.targets.get(&target)?.fields.get(field).cloned()
        }

        fn send_message(&mut self, obj: u32, msg: &str) {
            self.messages.push((obj, msg.to_string()));
        }

        fn is_particle_system(&self, target: u32) -> bool {
            self.targets
                .get(&target)
                .map(|t| t.kind == "ParticleSystem")
                .unwrap_or(false)
        }

        fn set_particle_start_lifetime(&mut self, target: u32, value: f32) {
            self.targets.get_mut(&target).unwrap().particle_lifetime = Some(value);
        }

        fn set_particle_start_speed(&mut self, target: u32, value: f32) {
            self.targets.get_mut(&target).unwrap().particle_speed = Some(value);
        }

        fn set_animation_curve(&mut self, target: u32, field: &str, keys: Vec<Keyframe>) {
            self.targets
                .get_mut(&target)
                .unwrap()
                .curves
                .insert(field.to_string(), keys);
        }

        fn set_array(
            &mut self,
            target: u32,
            field: &str,
            size: i32,
            elements: Vec<ArrayElement<Self>>,
        ) {
            self.targets
                .get_mut(&target)
                .unwrap()
                .arrays
                .insert(field.to_string(), (size, elements));
        }
    }

    impl Value<MockScene> {
        fn expect_f32(&self) -> f32 {
            match self {
                Value::Float(v) => *v,
                _ => panic!("expected Float, got {self:?}"),
            }
        }
        fn expect_i32(&self) -> i32 {
            match self {
                Value::Integer(v) | Value::Enum(v) => *v,
                _ => panic!("expected Integer/Enum, got {self:?}"),
            }
        }
        fn expect_vec3(&self) -> Vec3 {
            match self {
                Value::Vector3(v) => *v,
                _ => panic!("expected Vector3, got {self:?}"),
            }
        }
    }

    const SAMPLE: &str = "GameObject WindArea\n\tComponent UnityEngine.BoxCollider\n\t\tVector3 m_Size\n\t\t\tFloat x = 31.4\n\tComponent WindArea\n\t\tVector3 windDirectionHandle\n\t\t\tFloat x = 17.67106\n\t\t\tFloat y = 0.617309\n\t\t\tFloat z = 0\n\t\tFloat m_windPowerFactor = 0.26\n\tGameObject WindEffect1\n\t\tComponent UnityEngine.Transform\n\t\t\tQuaternion m_LocalRotation\n\t\t\t\tFloat x = 0.0005903003\n\t\t\t\tFloat y = 0.7071065\n\t\t\t\tFloat z = -0.0005903003\n\t\t\t\tFloat w = 0.7071065\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = -15.69998\n\t\t\t\tFloat y = 0.01111133\n\t\t\t\tFloat z = -2\n\t\tComponent UnityEngine.ParticleSystem\n\t\t\tGeneric InitialModule\n\t\t\t\tGeneric startLifetime\n\t\t\t\t\tFloat scalar = 5.233333\n\t\t\t\tGeneric startSpeed\n\t\t\t\t\tFloat scalar = 6\n\t\t\tGeneric EmissionModule\n\t\t\t\tGeneric rate\n\t\t\t\t\tFloat scalar = 1\n";

    #[test]
    fn end_to_end_wind_area_override_dispatches_through_state_machine() {
        let mut scene = MockScene::new();
        let root = scene.add_object("WindArea");
        let box_collider = scene.add_target(root, "BoxCollider");
        let wind = scene.add_target(root, "WindArea");
        let effect = scene.add_child(root, "WindEffect1");
        let transform = scene.add_target(effect, "Transform");
        scene.add_target(effect, "ParticleSystem");

        read_prefab_overrides(&mut scene, root, SAMPLE);

        // BoxCollider.m_Size read-modify-wrote a Vector3 with only `x` set.
        let size = scene.targets[&box_collider].fields["m_Size"].expect_vec3();
        assert!((size.x - 31.4).abs() < 1e-3);
        assert!(size.y.abs() < 1e-3);
        assert!(size.z.abs() < 1e-3);

        // WindArea component got its windDirectionHandle Vector3 + Float.
        let handle = scene.targets[&wind].fields["windDirectionHandle"].expect_vec3();
        assert!((handle.x - 17.67106).abs() < 1e-3);
        assert!((handle.y - 0.617309).abs() < 1e-3);
        let power = scene.targets[&wind].fields["m_windPowerFactor"].expect_f32();
        assert!((power - 0.26).abs() < 1e-3);

        // Transform on the child got Vector3/Quaternion.
        let pos = scene.targets[&transform].fields["m_LocalPosition"].expect_vec3();
        assert!((pos.x + 15.69998).abs() < 1e-3);
        match &scene.targets[&transform].fields["m_LocalRotation"] {
            Value::Quaternion(q) => {
                assert!((q.w - 0.7071065).abs() < 1e-4);
            }
            other => panic!("expected Quaternion, got {other:?}"),
        }

        // ParticleSystem got its start lifetime / speed via the module hook.
        let ps_id = scene.objects[&effect].components["ParticleSystem"];
        assert_eq!(scene.targets[&ps_id].particle_lifetime, Some(5.233333));
        assert_eq!(scene.targets[&ps_id].particle_speed, Some(6.0));

        // OnDataLoaded was sent to the root after the walk completed.
        assert_eq!(scene.messages, vec![(root, "OnDataLoaded".to_string())]);
    }

    const ARRAY_SAMPLE: &str = "GameObject BackgroundObject\n\tComponent PositionSerializer\n\t\tArray childLocalPositions\n\t\t\tArraySize size = 3\n\t\t\tElement 0\n\t\t\t\tVector3 data\n\t\t\t\tFloat y = 62.22481\n\t\t\t\tFloat z = 50\n\t\t\tElement 2\n\t\t\t\tVector3 data\n\t\t\t\tFloat z = -5\n";

    const FLAT_GENERIC_ARRAY_SAMPLE: &str = "GameObject LevelManager\n\tComponent LevelManager\n\t\tArray m_partTypeCounts\n\t\t\tArraySize size = 2\n\t\t\tElement 0\n\t\t\t\tGeneric data\n\t\t\t\tEnum type = 10\n\t\t\t\tInteger count = 1\n\t\t\tElement 1\n\t\t\t\tGeneric data\n\t\t\t\tEnum type = 1\n\t\t\t\tInteger count = 2\n\t\tBoolean m_darkLevel = True\n";

    #[test]
    fn read_array_collects_sparse_indices_with_value_type_elements() {
        let mut scene = MockScene::new();
        let root = scene.add_object("BackgroundObject");
        let serializer = scene.add_target(root, "PositionSerializer");

        read_prefab_overrides(&mut scene, root, ARRAY_SAMPLE);

        let (size, elements) = &scene.targets[&serializer].arrays["childLocalPositions"];
        assert_eq!(*size, 3);
        assert_eq!(elements.len(), 2);
        let first = elements[0].value.expect_vec3();
        assert!((first.y - 62.22481).abs() < 1e-3);
        assert!((first.z - 50.0).abs() < 1e-3);
        let last = elements[1].value.expect_vec3();
        assert_eq!(elements[1].index, 2);
        assert!((last.z + 5.0).abs() < 1e-3);
    }

    #[test]
    fn flat_generic_array_elements_do_not_drop_following_fields() {
        let mut scene = MockScene::new();
        let root = scene.add_object("LevelManager");
        let level_manager = scene.add_target(root, "LevelManager");

        read_prefab_overrides(&mut scene, root, FLAT_GENERIC_ARRAY_SAMPLE);

        let (size, elements) = &scene.targets[&level_manager].arrays["m_partTypeCounts"];
        assert_eq!(*size, 2);
        assert_eq!(elements.len(), 2);

        let Value::Generic(first) = &elements[0].value else {
            panic!("expected first array element to parse as Generic");
        };
        assert!(matches!(
            first.iter().find(|(name, _)| name == "type").map(|(_, value)| value),
            Some(Value::Enum(10))
        ));
        assert!(matches!(
            first.iter().find(|(name, _)| name == "count").map(|(_, value)| value),
            Some(Value::Integer(1))
        ));

        match &scene.targets[&level_manager].fields["m_darkLevel"] {
            Value::Boolean(true) => {}
            other => panic!("expected m_darkLevel Boolean(true), got {other:?}"),
        }
    }

    const ENUM_INT_SAMPLE: &str = "GameObject Root\n\tComponent Demo\n\t\tInteger count = 42\n\t\tEnum mode = 3\n\t\tBoolean flag = True\n";

    #[test]
    fn integer_enum_boolean_round_trip_through_set_field() {
        let mut scene = MockScene::new();
        let root = scene.add_object("Root");
        let demo = scene.add_target(root, "Demo");

        read_prefab_overrides(&mut scene, root, ENUM_INT_SAMPLE);

        assert_eq!(scene.targets[&demo].fields["count"].expect_i32(), 42);
        assert_eq!(scene.targets[&demo].fields["mode"].expect_i32(), 3);
        match &scene.targets[&demo].fields["flag"] {
            Value::Boolean(true) => {}
            other => panic!("expected Boolean(true), got {other:?}"),
        }
    }

    const READER_LINE: &str = "Float my long field name = 1.5\n";

    #[test]
    fn property_data_recombines_multi_word_field_names() {
        let mut reader = ObjectReader::new(READER_LINE);
        let prop = reader.read_property().unwrap();
        assert_eq!(prop.r#type, "Float");
        assert_eq!(prop.name, "my long field name");
        assert!((prop.float_value() - 1.5).abs() < 1e-6);
    }

    #[test]
    fn get_indentation_caches_tab_count_until_next_line() {
        let mut reader = ObjectReader::new("\t\tFloat x = 1\n");
        assert_eq!(reader.get_indentation(), 2);
        // Cached: a second call without read_line returns the same value
        // without consuming more bytes.
        assert_eq!(reader.get_indentation(), 2);
        let prop = reader.read_property().unwrap();
        assert_eq!(prop.name, "x");
        // After read_line, the next get_indentation re-scans (now zero).
        assert_eq!(reader.get_indentation(), 0);
    }
}
