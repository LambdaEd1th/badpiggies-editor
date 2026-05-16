//! Lightweight runtime host for Unity `OnDataLoaded` component hooks.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use crate::data::assets;
use crate::domain::prefab_asset::PrefabAssetDocument;
use crate::domain::prefab_override_runtime::{RuntimeOverrideDocument, RuntimeOverrideNode};

#[derive(Clone, Copy)]
pub struct RuntimeComponentContext<'a> {
    pub root: &'a RuntimeOverrideNode,
    pub game_object: &'a RuntimeOverrideNode,
    pub component: Option<&'a RuntimeOverrideNode>,
    pub component_suffix: &'a str,
}

pub trait RuntimeOnDataLoadedHook<T> {
    fn component_suffix(&self) -> &'static str;

    fn on_data_loaded(&self, context: RuntimeComponentContext<'_>, state: &mut T);
}

pub fn apply_runtime_on_data_loaded_hooks<T>(
    document: &RuntimeOverrideDocument,
    state: &mut T,
    hooks: &[&dyn RuntimeOnDataLoadedHook<T>],
) {
    apply_runtime_on_data_loaded_hooks_with_expected(document, state, &[], hooks);
}

pub fn apply_runtime_on_data_loaded_hooks_with_prefab_asset<T>(
    document: &RuntimeOverrideDocument,
    state: &mut T,
    prefab_asset_path: &str,
    hooks: &[&dyn RuntimeOnDataLoadedHook<T>],
) {
    let expected_components = prefab_root_component_suffixes(prefab_asset_path);
    let expected_refs: Vec<&str> = expected_components.iter().map(String::as_str).collect();
    apply_runtime_on_data_loaded_hooks_with_expected(document, state, &expected_refs, hooks);
}

pub fn apply_runtime_on_data_loaded_hooks_with_expected<T>(
    document: &RuntimeOverrideDocument,
    state: &mut T,
    expected_root_components: &[&str],
    hooks: &[&dyn RuntimeOnDataLoadedHook<T>],
) {
    for root in document.roots_of_type("GameObject") {
        dispatch_root_components(root, state, expected_root_components, hooks);
    }
}

fn dispatch_root_components<T>(
    root: &RuntimeOverrideNode,
    state: &mut T,
    expected_root_components: &[&str],
    hooks: &[&dyn RuntimeOnDataLoadedHook<T>],
) {
    for child in root.children() {
        if child.node_type == "Component" {
            let suffix = component_suffix(child);
            dispatch_component_hooks(
                RuntimeComponentContext {
                    root,
                    game_object: root,
                    component: Some(child),
                    component_suffix: suffix,
                },
                state,
                hooks,
            );
        }
    }

    for &suffix in expected_root_components {
        if root.component(suffix).is_none() {
            dispatch_component_hooks(
                RuntimeComponentContext {
                    root,
                    game_object: root,
                    component: None,
                    component_suffix: suffix,
                },
                state,
                hooks,
            );
        }
    }
}

fn dispatch_component_hooks<T>(
    context: RuntimeComponentContext<'_>,
    state: &mut T,
    hooks: &[&dyn RuntimeOnDataLoadedHook<T>],
) {
    for hook in hooks {
        if hook.component_suffix() == context.component_suffix {
            hook.on_data_loaded(context, state);
        }
    }
}

fn component_suffix(component: &RuntimeOverrideNode) -> &str {
    component
        .name
        .rsplit('.')
        .next()
        .unwrap_or(component.name.as_str())
}

fn prefab_root_component_suffixes(prefab_asset_path: &str) -> Vec<String> {
    static CACHE: OnceLock<Mutex<HashMap<String, Vec<String>>>> = OnceLock::new();

    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(cached) = cache
        .lock()
        .expect("prefab root component cache poisoned")
        .get(prefab_asset_path)
        .cloned()
    {
        return cached;
    }

    let parsed = assets::read_asset_text(prefab_asset_path)
        .and_then(|text| PrefabAssetDocument::parse(&text))
        .map(|prefab| prefab.root_component_suffixes())
        .unwrap_or_default();

    cache
        .lock()
        .expect("prefab root component cache poisoned")
        .insert(prefab_asset_path.to_string(), parsed.clone());

    parsed
}

#[cfg(test)]
mod tests {
    use super::{
        RuntimeComponentContext, RuntimeOnDataLoadedHook, apply_runtime_on_data_loaded_hooks,
        apply_runtime_on_data_loaded_hooks_with_expected,
        apply_runtime_on_data_loaded_hooks_with_prefab_asset,
    };
    use crate::domain::prefab_asset::PrefabAssetDocument;
    use crate::domain::prefab_override_runtime::RuntimeOverrideDocument;

    const SAMPLE_OVERRIDE: &str = "GameObject Root\n\tComponent Test.Namespace.PositionSerializer\n\tGameObject Child\n\t\tComponent Test.Namespace.PositionSerializer\n\tGameObject Other\n\t\tComponent Ignored\n";

    const ROOT_WITHOUT_COMPONENT_OVERRIDE: &str = "GameObject Root\n\tGameObject EndPoint\n\t\tComponent UnityEngine.Transform\n\t\t\tVector3 m_LocalPosition\n\t\t\t\tFloat x = 4.5\n";

    const ROOT_PREFAB_WITH_MONOBEHAVIOUR: &str = "%YAML 1.1\n%TAG !u! tag:unity3d.com,2011:\n--- !u!1001 &100100000\nPrefab:\n  m_RootGameObject: {fileID: 101}\n--- !u!1 &101\nGameObject:\n  m_Component:\n  - component: {fileID: 201}\n  - component: {fileID: 202}\n  m_Name: Root\n--- !u!4 &201\nTransform:\n  m_GameObject: {fileID: 101}\n--- !u!114 &202\nMonoBehaviour:\n  m_GameObject: {fileID: 101}\n  m_Script: {fileID: 11500000, guid: ae5f82fde6e6559b4e6280a34047fbb4, type: 3}\n";

    struct CollectHook;

    impl RuntimeOnDataLoadedHook<Vec<(String, String, Option<String>)>> for CollectHook {
        fn component_suffix(&self) -> &'static str {
            "PositionSerializer"
        }

        fn on_data_loaded(
            &self,
            context: RuntimeComponentContext<'_>,
            state: &mut Vec<(String, String, Option<String>)>,
        ) {
            state.push((
                context.root.name.clone(),
                context.game_object.name.clone(),
                context.component.map(|component| component.name.clone()),
            ));
        }
    }

    struct BridgeHook;

    impl RuntimeOnDataLoadedHook<Vec<(String, bool)>> for BridgeHook {
        fn component_suffix(&self) -> &'static str {
            "Bridge"
        }

        fn on_data_loaded(
            &self,
            context: RuntimeComponentContext<'_>,
            state: &mut Vec<(String, bool)>,
        ) {
            state.push((context.root.name.clone(), context.component.is_some()));
        }
    }

    #[test]
    fn applies_on_data_loaded_hooks_only_to_root_components() {
        let document = RuntimeOverrideDocument::parse(SAMPLE_OVERRIDE);
        let hook = CollectHook;
        let mut seen = Vec::new();

        apply_runtime_on_data_loaded_hooks(&document, &mut seen, &[&hook]);

        assert_eq!(
            seen,
            vec![(
                "Root".to_string(),
                "Root".to_string(),
                Some("Test.Namespace.PositionSerializer".to_string()),
            )]
        );
    }

    #[test]
    fn applies_expected_root_component_hooks_without_override_nodes() {
        let document = RuntimeOverrideDocument::parse(ROOT_WITHOUT_COMPONENT_OVERRIDE);
        let hook = BridgeHook;
        let mut seen = Vec::new();

        apply_runtime_on_data_loaded_hooks_with_expected(&document, &mut seen, &["Bridge"], &[&hook]);

        assert_eq!(seen, vec![("Root".to_string(), false)]);
    }

    #[test]
    fn parses_root_component_suffixes_from_prefab_yaml() {
        let suffixes = PrefabAssetDocument::parse(ROOT_PREFAB_WITH_MONOBEHAVIOUR)
            .expect("expected prefab")
            .root_component_suffixes();

        assert_eq!(suffixes, vec!["Transform".to_string(), "PositionSerializer".to_string()]);
    }

    #[test]
    fn applies_root_component_hooks_from_prefab_asset_without_override_nodes() {
        let document = RuntimeOverrideDocument::parse(ROOT_WITHOUT_COMPONENT_OVERRIDE);
        let hook = BridgeHook;
        let mut seen = Vec::new();

        apply_runtime_on_data_loaded_hooks_with_prefab_asset(
            &document,
            &mut seen,
            "unity/prefabs/Bridge.prefab",
            &[&hook],
        );

        assert_eq!(seen, vec![("Root".to_string(), false)]);
    }
}