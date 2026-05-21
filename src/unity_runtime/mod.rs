//! Minimal Unity runtime replication backing the Rust port of
//! `ObjectDeserializer` / `LevelLoader.ReadPrefabOverrides`.
//!
//! Phasing:
//! - **P0** — scaffolding: `Scene`, `GameObject`, `Box<dyn UnityComponent>`
//!   storage, `UnknownComponent` round-trip catch-all, `Transform` /
//!   `BoxCollider` / `Behaviour` ports.
//! - **P1 (current)** — port Bad-Piggies-specific MonoBehaviours
//!   (WindArea / Bridge / Fan / Engine / BezierCurve / BezierMesh /
//!   PositionSerializer / PointLightSource / Sprite / UnmanagedSprite /
//!   ParticleSystem subset / Camera / Rigidbody).
//! - **P2+** — migrate every consumer that currently queries
//!   `OverrideNode` ASTs to read typed state off the [`Scene`] instead.

pub mod components;
pub mod reflection;
pub mod scene;

#[allow(unused_imports)]
pub use components::{Behaviour, BoxCollider, Transform, UnityComponent, UnknownComponent};
#[allow(unused_imports)]
pub use scene::{Component, ComponentId, GameObject, GameObjectId, Scene, SceneValue};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::object_deserializer::read_prefab_overrides;

    #[test]
    fn parses_transform_and_box_collider_into_typed_scene() {
        let mut scene = Scene::new();
        let root = scene.create_game_object("Root", None);
        scene.attach_component(root, Box::new(Transform::default()));
        scene.attach_component(root, Box::new(BoxCollider::default()));

        let text = "\
GameObject Root
\tComponent Transform
\t\tVector3 m_LocalPosition
\t\t\tFloat x = 1.5
\t\t\tFloat y = -2.25
\t\t\tFloat z = 0
\t\tVector3 m_LocalScale
\t\t\tFloat x = 2
\t\t\tFloat y = 2
\t\t\tFloat z = 2
\tComponent BoxCollider
\t\tVector3 m_Size
\t\t\tFloat x = 3
\t\t\tFloat y = 4
\t\t\tFloat z = 5
";

        read_prefab_overrides(&mut scene, root, text);

        let t = scene.transform(root).expect("Transform attached");
        let pos = t.local_position.expect("position set");
        assert_eq!(pos.x, 1.5);
        assert_eq!(pos.y, -2.25);
        assert_eq!(pos.z, 0.0);
        let scale = t.local_scale.expect("scale set");
        assert_eq!(scale.x, 2.0);
        assert_eq!(scale.y, 2.0);

        let bc_id = scene.get_component(root, "BoxCollider").expect("BoxCollider");
        let bc = scene
            .component_as::<BoxCollider>(bc_id)
            .expect("downcast BoxCollider");
        let size = bc.size.expect("size set");
        assert_eq!(size.x, 3.0);
        assert_eq!(size.y, 4.0);
        assert_eq!(size.z, 5.0);
    }

    #[test]
    fn unknown_component_preserves_writes() {
        let mut scene = Scene::new();
        let root = scene.create_game_object("Root", None);

        let text = "\
GameObject Root
\tComponent FancyMonoBehaviour
\t\tInteger answer = 42
\t\tFloat ratio = 0.5
";
        read_prefab_overrides(&mut scene, root, text);

        let cid = scene
            .get_component(root, "FancyMonoBehaviour")
            .expect("unknown component allocated by add_component");
        let uc = scene
            .component_as::<UnknownComponent>(cid)
            .expect("downcast UnknownComponent");
        assert_eq!(uc.suffix, "FancyMonoBehaviour");
        assert_eq!(uc.fields.len(), 2);
        assert_eq!(uc.fields[0].0, "answer");
        assert!(matches!(uc.fields[0].1, SceneValue::Integer(42)));
        assert_eq!(uc.fields[1].0, "ratio");
        assert!(matches!(uc.fields[1].1, SceneValue::Float(v) if v == 0.5));
    }
}
