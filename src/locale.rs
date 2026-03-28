use std::sync::LazyLock;

use fluent_bundle::{FluentBundle, FluentResource};

/// Supported UI languages.
#[derive(Default, Clone, Copy, PartialEq, Eq)]
pub enum Language {
    #[default]
    Zh,
    En,
}

/// Runtime i18n bundle wrapping a Fluent resource.
pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

// SAFETY: FluentBundle uses Rc internally and is not Send/Sync.
// Our statics are written once (LazyLock) and only read afterwards.
unsafe impl Send for I18n {}
unsafe impl Sync for I18n {}

impl I18n {
    fn new(source: &'static str, lang_tag: &'static str) -> Self {
        let res = FluentResource::try_new(source.to_owned())
            .expect("FTL parse error");
        let lid: unic_langid::LanguageIdentifier =
            lang_tag.parse().expect("invalid lang tag");
        let mut bundle = FluentBundle::new(vec![lid]);
        bundle.add_resource(res).expect("FTL add_resource error");
        I18n { bundle }
    }

    /// Look up a simple (no-argument) message by key.
    pub fn get(&self, key: &str) -> String {
        let Some(msg) = self.bundle.get_message(key) else {
            return format!("[missing: {key}]");
        };
        let Some(pattern) = msg.value() else {
            return format!("[no-value: {key}]");
        };
        let mut errors = vec![];
        self.bundle
            .format_pattern(pattern, None, &mut errors)
            .into_owned()
    }

    /// Format the "loaded" status message with object / root counts.
    pub fn fmt_status_loaded(&self, obj_count: usize, root_count: usize) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("obj_count", obj_count as i64);
        args.set("root_count", root_count as i64);
        self.format_with("status_loaded", &args)
    }

    /// Format a single-argument message; the FTL argument is always named `$name`.
    pub fn fmt1(&self, key: &str, name: &str) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("name", name.to_owned());
        self.format_with(key, &args)
    }

    fn format_with(&self, key: &str, args: &fluent_bundle::FluentArgs) -> String {
        let Some(msg) = self.bundle.get_message(key) else {
            return format!("[missing: {key}]");
        };
        let Some(pattern) = msg.value() else {
            return format!("[no-value: {key}]");
        };
        let mut errors = vec![];
        self.bundle
            .format_pattern(pattern, Some(args), &mut errors)
            .into_owned()
    }
}

static ZH_I18N: LazyLock<I18n> =
    LazyLock::new(|| I18n::new(include_str!("../locales/zh-CN.ftl"), "zh-CN"));

static EN_I18N: LazyLock<I18n> =
    LazyLock::new(|| I18n::new(include_str!("../locales/en-US.ftl"), "en-US"));

impl Language {
    pub fn i18n(self) -> &'static I18n {
        match self {
            Language::Zh => &ZH_I18N,
            Language::En => &EN_I18N,
        }
    }

    /// Native name shown in the language picker (always in its own language).
    pub fn display_name(self) -> &'static str {
        match self {
            Language::Zh => "中文",
            Language::En => "English",
        }
    }

    /// All available languages, in menu order.
    pub const ALL: &'static [Language] = &[Language::Zh, Language::En];
}
