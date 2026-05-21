//! Catch-all for component types that don't have a typed port yet.
//!
//! Preserves every property write in stream order so round-trip
//! serialization (Phase P3) can re-emit the override block byte-identical to
//! the input.

use crate::unity_component_boilerplate;
use crate::unity_runtime::components::UnityComponent;
use crate::unity_runtime::scene::{Scene, SceneValue};

#[derive(Debug, Clone)]
pub struct UnknownComponent {
    pub suffix: String,
    /// `(field_name, value)` in stream order.
    pub fields: Vec<(String, SceneValue)>,
}

impl UnknownComponent {
    pub fn new(suffix: impl Into<String>) -> Self {
        Self {
            suffix: suffix.into(),
            fields: Vec::new(),
        }
    }

    /// Most recent write for `name`, or `None` if never set.
    pub fn lookup(&self, name: &str) -> Option<SceneValue> {
        self.fields
            .iter()
            .rev()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.clone())
    }
}

impl UnityComponent for UnknownComponent {
    fn component_suffix(&self) -> &str {
        &self.suffix
    }

    fn get_field(&self, _scene: &Scene, name: &str) -> Option<SceneValue> {
        self.lookup(name)
    }

    fn set_field(&mut self, _scene: &mut Scene, name: &str, value: SceneValue) -> bool {
        // Accept everything and record it; returning `true` keeps the host
        // from double-recording via `extra_mut`.
        self.fields.push((name.to_string(), value));
        true
    }

    fn extra_mut(&mut self) -> &mut Vec<(String, SceneValue)> {
        &mut self.fields
    }

    fn extra(&self) -> &[(String, SceneValue)] {
        &self.fields
    }

    unity_component_boilerplate!();
}
