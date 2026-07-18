//! Arena-backed Unity-style scene graph.
//!
//! All `GameObject`s and `Component`s live in two parallel `Vec`s and are
//! referenced by `GameObjectId` / `ComponentId` indices, matching the
//! reflective handle types declared by
//! [`RuntimeHost`](crate::domain::object_deserializer::RuntimeHost) (which
//! require `Copy + Eq`).

use crate::domain::object_deserializer::Value;
use crate::unity_runtime::components::{UnityComponent, UnknownComponent};

/// `Value<H>` where `H = Scene`. The exact same enum the state machine emits,
/// just with the concrete scene handle type bound in.
pub type SceneValue = Value<Scene>;

/// Opaque handle to a `GameObject` in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GameObjectId(pub(crate) u32);

/// Opaque handle to a `Component` in the scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ComponentId(pub(crate) u32);

#[derive(Debug, Clone)]
pub struct GameObject {
    pub name: String,
    pub parent: Option<GameObjectId>,
    pub children: Vec<GameObjectId>,
    pub components: Vec<ComponentId>,
}

#[derive(Debug, Clone)]
pub struct Component {
    pub owner: GameObjectId,
    pub behavior: Box<dyn UnityComponent>,
}

/// Lightweight Unity scene. Stores GameObjects + Components in arenas.
///
/// The state machine consumes this through
/// [`impl RuntimeHost for Scene`](crate::unity_runtime::reflection).
#[derive(Debug, Default, Clone)]
pub struct Scene {
    game_objects: Vec<GameObject>,
    components: Vec<Component>,
    /// When `true`, the `RuntimeHost` impl auto-creates missing child
    /// GameObjects on `find_child` (rather than returning `None` and letting
    /// the deserializer drop the sub-tree). Used by
    /// [`Scene::from_override_text`] so the state machine can synthesize a
    /// full scene graph from override text alone, without a pre-loaded
    /// prefab instance.
    pub(crate) permissive: bool,
}

impl Scene {
    pub fn new() -> Self {
        Self::default()
    }

    // ----- Allocation -------------------------------------------------------

    /// Allocate a new GameObject. If `parent` is `Some`, the new object is
    /// added to that parent's `children` list and inherits a `parent` link.
    pub fn create_game_object(
        &mut self,
        name: impl Into<String>,
        parent: Option<GameObjectId>,
    ) -> GameObjectId {
        let id = GameObjectId(self.game_objects.len() as u32);
        self.game_objects.push(GameObject {
            name: name.into(),
            parent,
            children: Vec::new(),
            components: Vec::new(),
        });
        if let Some(parent_id) = parent {
            self.game_objects[parent_id.0 as usize].children.push(id);
        }
        id
    }

    /// Allocate a new component on `owner` with the given typed payload.
    pub fn attach_component(
        &mut self,
        owner: GameObjectId,
        behavior: Box<dyn UnityComponent>,
    ) -> ComponentId {
        let id = ComponentId(self.components.len() as u32);
        self.components.push(Component { owner, behavior });
        self.game_objects[owner.0 as usize].components.push(id);
        id
    }

    /// Convenience: allocate an [`UnknownComponent`] (catch-all for component
    /// suffixes that don't have a typed port yet).
    pub fn attach_unknown_component(
        &mut self,
        owner: GameObjectId,
        suffix: impl Into<String>,
    ) -> ComponentId {
        self.attach_component(owner, Box::new(UnknownComponent::new(suffix)))
    }

    // ----- Query ------------------------------------------------------------

    pub fn game_object(&self, id: GameObjectId) -> &GameObject {
        &self.game_objects[id.0 as usize]
    }

    pub fn game_object_mut(&mut self, id: GameObjectId) -> &mut GameObject {
        &mut self.game_objects[id.0 as usize]
    }

    pub fn component(&self, id: ComponentId) -> &Component {
        &self.components[id.0 as usize]
    }

    pub fn component_mut(&mut self, id: ComponentId) -> &mut Component {
        &mut self.components[id.0 as usize]
    }

    pub fn behavior(&self, id: ComponentId) -> &dyn UnityComponent {
        self.components[id.0 as usize].behavior.as_ref()
    }

    pub fn behavior_mut(&mut self, id: ComponentId) -> &mut dyn UnityComponent {
        self.components[id.0 as usize].behavior.as_mut()
    }

    /// Downcast the component's behavior to a concrete type.
    pub fn component_as<T: UnityComponent>(&self, id: ComponentId) -> Option<&T> {
        self.behavior(id).as_any().downcast_ref::<T>()
    }

    pub fn component_as_mut<T: UnityComponent>(&mut self, id: ComponentId) -> Option<&mut T> {
        self.behavior_mut(id).as_any_mut().downcast_mut::<T>()
    }

    pub fn iter_game_objects(&self) -> impl Iterator<Item = (GameObjectId, &GameObject)> {
        self.game_objects
            .iter()
            .enumerate()
            .map(|(i, go)| (GameObjectId(i as u32), go))
    }

    pub fn iter_components(&self) -> impl Iterator<Item = (ComponentId, &Component)> {
        self.components
            .iter()
            .enumerate()
            .map(|(i, c)| (ComponentId(i as u32), c))
    }

    /// `transform.Find(name)` — first direct child with the given name.
    pub fn find_child(&self, parent: GameObjectId, name: &str) -> Option<GameObjectId> {
        let parent = self.game_object(parent);
        parent
            .children
            .iter()
            .find(|&&child_id| self.game_object(child_id).name == name)
            .copied()
    }

    /// `obj.GetComponent(suffix)` — first component whose `component_suffix`
    /// matches.
    pub fn get_component(&self, owner: GameObjectId, suffix: &str) -> Option<ComponentId> {
        let owner = self.game_object(owner);
        owner
            .components
            .iter()
            .find(|&&component_id| self.behavior(component_id).component_suffix() == suffix)
            .copied()
    }

    /// First component on `owner` that downcasts to `T`.
    pub fn get_component_of<T: UnityComponent>(
        &self,
        owner: GameObjectId,
    ) -> Option<(ComponentId, &T)> {
        for &cid in &self.game_object(owner).components {
            if let Some(t) = self.component_as::<T>(cid) {
                return Some((cid, t));
            }
        }
        None
    }

    /// Synthesize a [`Scene`] from prefab-override text in "permissive" mode:
    /// any child `GameObject` referenced by the text is auto-created (rather
    /// than dropped), so consumers can read the resulting typed components
    /// without first having to instantiate a real prefab. Returns the scene
    /// plus the root `GameObjectId`.
    ///
    /// Returns `None` if the text does not begin with a `GameObject <name>`
    /// header line.
    pub fn from_override_text(text: &str) -> Option<(Self, GameObjectId)> {
        // Tolerate a UTF-8 BOM at the very start of the override text — the
        // bundled `*.bytes` blobs include one and the deserializer's line
        // reader does not strip it on its own.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        // If the override text starts directly with a `Component …` block
        // (no GameObject header — some prefab-attached overrides ship this
        // way), synthesize a placeholder root so the deserializer has
        // somewhere to attach those components.
        let (root_name, owned_buf): (String, Option<String>) =
            match peek_root_game_object_name(text) {
                Some(name) => (name, None),
                None if peek_starts_with_kind(text, "Component") => (
                    "__synthetic_root".to_string(),
                    Some({
                        // Indent every original line one extra tab so the
                        // bare Component blocks sit inside the synthetic
                        // GameObject root at the depth the deserializer
                        // expects.
                        let mut buf = String::from("GameObject __synthetic_root\n");
                        for line in text.lines() {
                            buf.push('\t');
                            buf.push_str(line);
                            buf.push('\n');
                        }
                        buf
                    }),
                ),
                None => return None,
            };
        let text_for_reader: &str = owned_buf.as_deref().unwrap_or(text);
        let mut scene = Scene::new();
        scene.permissive = true;
        let root = scene.create_game_object(root_name, None);
        crate::domain::object_deserializer::read_prefab_overrides(
            &mut scene,
            root,
            text_for_reader,
        );
        scene.permissive = false;
        Some((scene, root))
    }
}

/// Find the first non-empty, zero-indentation line of the form
/// `GameObject <name>` and return `<name>`.
fn peek_root_game_object_name(text: &str) -> Option<String> {
    for line in text.lines() {
        if line.starts_with('\t') {
            continue;
        }
        let trimmed = line.trim_end_matches(['\r']);
        if trimmed.is_empty() {
            continue;
        }
        let mut parts = trimmed.splitn(2, ' ');
        let kind = parts.next()?;
        if kind != "GameObject" {
            return None;
        }
        let name = parts.next()?;
        return Some(name.to_string());
    }
    None
}

/// First non-empty, zero-indent line begins with `<kind> `.
fn peek_starts_with_kind(text: &str, kind: &str) -> bool {
    for line in text.lines() {
        if line.starts_with('\t') {
            continue;
        }
        let trimmed = line.trim_end_matches(['\r']);
        if trimmed.is_empty() {
            continue;
        }
        return trimmed
            .split_once(' ')
            .map(|(head, _)| head == kind)
            .unwrap_or(false);
    }
    false
}
