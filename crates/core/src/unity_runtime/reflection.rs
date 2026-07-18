//! `impl RuntimeHost for Scene` — bridges the `ObjectDeserializer` state
//! machine to the typed [`Scene`] storage.
//!
//! Reflection re-entrancy uses [`std::mem::replace`]: each `set_field` /
//! `on_data_loaded` call temporarily lifts the component's behavior out of
//! the arena (leaving a placeholder), runs the typed method with full
//! `&mut Scene` access, then puts the behavior back.

use crate::domain::object_deserializer::{ArrayElement, Keyframe, RuntimeHost, Value};
use crate::unity_runtime::components::{
    ParticleSystem, Transform, UnityComponent, UnknownComponent, make_component_by_suffix,
};
use crate::unity_runtime::scene::{ComponentId, GameObjectId, Scene, SceneValue};

impl RuntimeHost for Scene {
    type GameObject = GameObjectId;
    type Target = ComponentId;

    fn referenced_object(&self, _index: i32) -> Option<Value<Self>> {
        // Reference table is populated by the LevelLoader-side pipeline; the
        // bare Scene has no references.
        None
    }

    fn find_child(&mut self, parent: Self::GameObject, name: &str) -> Option<Self::GameObject> {
        if let Some(found) = Scene::find_child(self, parent, name) {
            return Some(found);
        }
        // Permissive mode (set by `Scene::from_override_text`): synthesize
        // missing children so the deserializer doesn't drop their sub-tree.
        if self.permissive {
            return Some(self.create_game_object(name, Some(parent)));
        }
        None
    }

    fn get_component(&mut self, obj: Self::GameObject, name: &str) -> Option<Self::Target> {
        Scene::get_component(self, obj, name)
    }

    fn add_component(&mut self, obj: Self::GameObject, name: &str) -> Option<Self::Target> {
        let behavior =
            make_component_by_suffix(name).unwrap_or_else(|| Box::new(UnknownComponent::new(name)));
        Some(self.attach_component(obj, behavior))
    }

    fn game_object_name(&self, obj: Self::GameObject) -> String {
        self.game_object(obj).name.clone()
    }

    fn set_field(&mut self, target: Self::Target, field: &str, value: Value<Self>) {
        with_behavior(self, target, |scene, b| {
            if !b.set_field(scene, field, value.clone()) {
                b.extra_mut().push((field.to_string(), value));
            }
        });
    }

    fn get_field(&mut self, target: Self::Target, field: &str) -> Option<Value<Self>> {
        // Clone the behavior out, query against the (immutable) Scene through
        // the &mut path. `get_field` is read-only so no write-back is needed.
        let clone = self.behavior(target).clone_box();
        clone.get_field(self, field)
    }

    fn send_message(&mut self, obj: Self::GameObject, message: &str) {
        if message != "OnDataLoaded" {
            return;
        }
        // BroadcastMessage-style propagation: walk receiver + descendants.
        let mut stack = vec![obj];
        while let Some(go) = stack.pop() {
            let component_ids: Vec<ComponentId> = self.game_object(go).components.clone();
            for cid in component_ids {
                with_behavior(self, cid, |scene, b| b.on_data_loaded(scene));
            }
            stack.extend(self.game_object(go).children.iter().copied());
        }
    }

    fn set_animation_curve(&mut self, target: Self::Target, field: &str, keys: Vec<Keyframe>) {
        // Stash on `extra` via the regular set_field path; typed components
        // that own a curve field override `set_field` to intercept.
        let value = SceneValue::Generic(
            keys.into_iter()
                .enumerate()
                .map(|(i, k)| (i.to_string(), SceneValue::Keyframe(k)))
                .collect(),
        );
        self.set_field(target, field, value);
    }

    fn set_array(
        &mut self,
        target: Self::Target,
        field: &str,
        size: i32,
        elements: Vec<ArrayElement<Self>>,
    ) {
        let mut entries: Vec<(String, SceneValue)> = Vec::with_capacity(elements.len() + 1);
        entries.push(("size".to_string(), SceneValue::Integer(size)));
        for el in elements {
            entries.push((el.index.to_string(), el.value));
        }
        self.set_field(target, field, SceneValue::Generic(entries));
    }

    fn set_object_reference_index(&mut self, target: Self::Target, field: &str, index: i32) {
        with_behavior(self, target, |scene, b| {
            if !b.set_object_reference_index(scene, field, index) {
                // Spill an explicit "(field, ObjectReferenceIndex(idx))"
                // tuple — represented as a synthetic Generic so it survives
                // round-trip serialization in P3.
                b.extra_mut().push((
                    field.to_string(),
                    SceneValue::Generic(vec![(
                        "_unresolvedObjectReference".to_string(),
                        SceneValue::Integer(index),
                    )]),
                ));
            }
        });
    }

    // ---------- ParticleSystem reflective hooks --------------------------

    fn is_particle_system(&self, target: Self::Target) -> bool {
        self.component_as::<ParticleSystem>(target).is_some()
    }

    fn set_particle_start_lifetime(&mut self, target: Self::Target, value: f32) {
        if let Some(ps) = self.component_as_mut::<ParticleSystem>(target) {
            ps.start_lifetime = Some(value);
        }
    }

    fn set_particle_start_speed(&mut self, target: Self::Target, value: f32) {
        if let Some(ps) = self.component_as_mut::<ParticleSystem>(target) {
            ps.start_speed = Some(value);
        }
    }

    fn set_particle_emission_rate(&mut self, target: Self::Target, value: f32) {
        if let Some(ps) = self.component_as_mut::<ParticleSystem>(target) {
            ps.emission_rate = Some(value);
        }
    }
}

/// Lift `target`'s behavior out of the arena, run `f` against it with the
/// rest of the `Scene` borrowed mutably, then put it back.
///
/// While the closure runs the slot holds a placeholder `UnknownComponent`
/// with the original suffix — observable only by re-entrant lookups that
/// touch the same component (which today's typed callbacks don't do).
fn with_behavior<R>(
    scene: &mut Scene,
    target: ComponentId,
    f: impl FnOnce(&mut Scene, &mut Box<dyn UnityComponent>) -> R,
) -> R {
    let suffix = scene.behavior(target).component_suffix().to_string();
    let placeholder: Box<dyn UnityComponent> = Box::new(UnknownComponent::new(suffix));
    let mut behavior = std::mem::replace(&mut scene.component_mut(target).behavior, placeholder);
    let result = f(scene, &mut behavior);
    scene.component_mut(target).behavior = behavior;
    result
}

// ---------------------------------------------------------------------------
// Convenience helpers
// ---------------------------------------------------------------------------

impl Scene {
    /// Look up the [`Transform`] on `obj`. Returns `None` if not attached.
    pub fn transform(&self, obj: GameObjectId) -> Option<&Transform> {
        self.get_component_of::<Transform>(obj).map(|(_, t)| t)
    }
}
