use std::sync::OnceLock;

use fluent_bundle::{FluentResource, concurrent::FluentBundle};

pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

impl I18n {
    fn english() -> Self {
        let source = badpiggies_editor_core::data::runtime_assets::read_runtime_asset_text(
            "locales/en-US.ftl",
        );
        let resource = FluentResource::try_new(source).unwrap_or_else(|(resource, errors)| {
            log::error!("FTL parse error: {errors:?}");
            resource
        });
        let language = "en-US".parse().expect("valid en-US language tag");
        let mut bundle = FluentBundle::new_concurrent(vec![language]);
        if let Err(errors) = bundle.add_resource(resource) {
            log::error!("FTL resource error: {errors:?}");
        }
        Self { bundle }
    }

    pub fn get(&self, key: &str) -> String {
        let Some(message) = self.bundle.get_message(key) else {
            return key.to_string();
        };
        let Some(pattern) = message.value() else {
            return key.to_string();
        };
        let mut errors = Vec::new();
        self.bundle
            .format_pattern(pattern, None, &mut errors)
            .into_owned()
    }
}

pub fn english() -> &'static I18n {
    static INSTANCE: OnceLock<I18n> = OnceLock::new();
    INSTANCE.get_or_init(I18n::english)
}
