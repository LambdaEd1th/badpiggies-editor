//! Typed Unity components.
//!
//! Storage is `Box<dyn UnityComponent>`. Adding a new component type is a
//! one-file change: implement [`UnityComponent`] for the new struct and
//! register it in [`make_component_by_suffix`].
//!
//! Typed consumer access goes through downcasting:
//! ```ignore
//! if let Some(wind) = scene.component_as::<WindArea>(id) { ... }
//! ```

use std::any::Any;
use std::fmt::Debug;

use crate::unity_runtime::scene::{Scene, SceneValue};

pub mod behaviour;
pub mod bezier_curve;
pub mod bezier_mesh;
pub mod box_collider;
pub mod bridge;
pub mod camera;
pub mod camera_preview;
pub mod engine;
pub mod fan;
pub mod level_manager;
pub mod particle_system;
pub mod point_light_source;
pub mod position_serializer;
pub mod rigidbody;
pub mod sprite;
pub mod transform;
pub mod unknown;
pub mod unmanaged_sprite;
pub mod wind_area;

pub use behaviour::Behaviour;
pub use bezier_curve::BezierCurve;
pub use bezier_mesh::BezierMesh;
pub use box_collider::BoxCollider;
pub use bridge::Bridge;
pub use camera::Camera;
pub use camera_preview::CameraPreview;
pub use engine::Engine;
pub use fan::Fan;
pub use level_manager::LevelManager;
pub use particle_system::ParticleSystem;
pub use point_light_source::PointLightSource;
pub use position_serializer::PositionSerializer;
pub use rigidbody::Rigidbody;
pub use sprite::Sprite;
pub use transform::Transform;
pub use unknown::UnknownComponent;
pub use unmanaged_sprite::UnmanagedSprite;
pub use wind_area::WindArea;

/// Trait implemented by every typed component variant.
///
/// `Any` enables downcasting; `clone_box` is the manual `dyn`-safe
/// replacement for `Clone`.
pub trait UnityComponent: Any + Debug {
    /// The trailing segment of the dot-qualified Unity type name (e.g.
    /// `"Transform"`, `"WindArea"`).
    fn component_suffix(&self) -> &str;

    fn get_field(&self, _scene: &Scene, _name: &str) -> Option<SceneValue> {
        None
    }
    fn set_field(&mut self, _scene: &mut Scene, _name: &str, _value: SceneValue) -> bool {
        false
    }

    /// Hook for `ObjectReference` writes the host couldn't resolve to a
    /// concrete `GameObject` — Unity drops them, but the editor often wants
    /// to remember the raw asset index so it can re-emit the override
    /// verbatim. Default: no-op (caller spills nothing).
    fn set_object_reference_index(&mut self, _scene: &mut Scene, _name: &str, _index: i32) -> bool {
        false
    }

    /// `MonoBehaviour.OnDataLoaded` post-processing hook.
    fn on_data_loaded(&mut self, _scene: &mut Scene) {}

    /// Unrecognized field writes spill here so round-trip serialization
    /// (Phase P3) re-emits them verbatim.
    fn extra_mut(&mut self) -> &mut Vec<(String, SceneValue)>;
    fn extra(&self) -> &[(String, SceneValue)];

    /// Manual clone (`dyn`-safe replacement for `Clone`). Each implementor
    /// returns `Box::new(self.clone())`.
    fn clone_box(&self) -> Box<dyn UnityComponent>;

    fn as_any(&self) -> &dyn Any;
    fn as_any_mut(&mut self) -> &mut dyn Any;
}

impl Clone for Box<dyn UnityComponent> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// Implement the `clone_box` / `as_any` / `as_any_mut` boilerplate.
/// Use inside an `impl UnityComponent for $T` block.
#[macro_export]
macro_rules! unity_component_boilerplate {
    () => {
        fn clone_box(
            &self,
        ) -> ::std::boxed::Box<dyn $crate::unity_runtime::components::UnityComponent> {
            ::std::boxed::Box::new(::std::clone::Clone::clone(self))
        }
        fn as_any(&self) -> &dyn ::std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn ::std::any::Any {
            self
        }
    };
}

/// Look up a typed-factory by the short component suffix written in the
/// override stream (e.g. `"Transform"`, `"BoxCollider"`, `"WindArea"`).
///
/// Returns `None` if the suffix isn't recognized — the caller falls back to
/// [`UnknownComponent`] so the data still round-trips.
pub fn make_component_by_suffix(suffix: &str) -> Option<Box<dyn UnityComponent>> {
    Some(match suffix {
        "Transform" => Box::new(Transform::default()),
        "BoxCollider" | "BoxCollider2D" => Box::new(BoxCollider::default()),
        "Behaviour" => Box::new(Behaviour::default()),
        "WindArea" => Box::new(WindArea::default()),
        "Bridge" => Box::new(Bridge::default()),
        "Fan" => Box::new(Fan::default()),
        "Engine" => Box::new(Engine::default()),
        "LevelManager" => Box::new(LevelManager::default()),
        "BezierCurve" => Box::new(BezierCurve::default()),
        "BezierMesh" => Box::new(BezierMesh::default()),
        "PositionSerializer" => Box::new(PositionSerializer::default()),
        "PointLightSource" => Box::new(PointLightSource::default()),
        "Sprite" => Box::new(Sprite::default()),
        "UnmanagedSprite" => Box::new(UnmanagedSprite::default()),
        "ParticleSystem" => Box::new(ParticleSystem::default()),
        "Camera" => Box::new(Camera::default()),
        "CameraPreview" => Box::new(CameraPreview::default()),
        "Rigidbody" => Box::new(Rigidbody::default()),
        _ => return None,
    })
}
