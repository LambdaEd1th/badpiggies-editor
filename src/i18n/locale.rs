use std::sync::LazyLock;

use fluent_bundle::{FluentResource, concurrent::FluentBundle};

/// UI language resolved from runtime locale registry.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Language {
    tag: &'static str,
}

/// Runtime i18n bundle wrapping a Fluent resource.
pub struct I18n {
    bundle: FluentBundle<FluentResource>,
}

impl I18n {
    fn new(source: String, lang_tag: &'static str) -> Self {
        let res = match FluentResource::try_new(source.to_owned()) {
            Ok(resource) => resource,
            Err((resource, errors)) => {
                log::error!("FTL parse error for {lang_tag}: {errors:?}");
                resource
            }
        };

        let lid: unic_langid::LanguageIdentifier = match lang_tag.parse() {
            Ok(lid) => lid,
            Err(error) => {
                log::error!("invalid language tag {lang_tag}: {error}");
                return I18n {
                    bundle: FluentBundle::new_concurrent(
                        Vec::<unic_langid::LanguageIdentifier>::new(),
                    ),
                };
            }
        };

        let mut bundle = FluentBundle::new_concurrent(vec![lid]);
        if let Err(errors) = bundle.add_resource(res) {
            log::error!("FTL add_resource error for {lang_tag}: {errors:?}");
        }
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

    /// Format a message with `$name` and `$count` arguments.
    pub fn fmt_name_count(&self, key: &str, name: &str, count: usize) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("name", name.to_owned());
        args.set("count", count as i64);
        self.format_with(key, &args)
    }

    /// Format a message with a single `$idx` argument.
    pub fn fmt_idx(&self, key: &str, idx: usize) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("idx", idx as i64);
        self.format_with(key, &args)
    }

    /// Format a save viewer status message with localized file type and byte count.
    pub fn fmt_save_viewer_type_bytes(&self, file_type: &str, bytes: usize) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("type", file_type.to_owned());
        args.set("bytes", bytes as i64);
        self.format_with("save_viewer_status_type_bytes", &args)
    }

    /// Format a save viewer status message with file name, localized file type, and byte count.
    pub fn fmt_save_viewer_file_type_bytes(
        &self,
        file_name: &str,
        file_type: &str,
        bytes: usize,
    ) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("file_name", file_name.to_owned());
        args.set("type", file_type.to_owned());
        args.set("bytes", bytes as i64);
        self.format_with("save_viewer_status_file_type_bytes", &args)
    }

    /// Format a two-argument message with `$path` and `$error`.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fmt_path_error(&self, key: &str, path: &str, error: &str) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("path", path.to_owned());
        args.set("error", error.to_owned());
        self.format_with(key, &args)
    }

    /// Format the CLI convert success message.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fmt_convert_ok(
        &self,
        input: &str,
        output: &str,
        obj_count: usize,
        root_count: usize,
    ) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("input", input.to_owned());
        args.set("output", output.to_owned());
        args.set("obj_count", obj_count as i64);
        args.set("root_count", root_count as i64);
        self.format_with("cli_convert_ok", &args)
    }

    /// Format the CLI decrypt success message.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fmt_cli_decrypt_ok(
        &self,
        input: &str,
        file_type: &str,
        output: &str,
        bytes: usize,
    ) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("input", input.to_owned());
        args.set("type", file_type.to_owned());
        args.set("output", output.to_owned());
        args.set("bytes", bytes as i64);
        self.format_with("cli_decrypt_ok", &args)
    }

    /// Format the CLI encrypt success message.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fmt_cli_encrypt_ok(
        &self,
        input: &str,
        output: &str,
        file_type: &str,
        bytes: usize,
    ) -> String {
        use fluent_bundle::FluentArgs;
        let mut args = FluentArgs::new();
        args.set("input", input.to_owned());
        args.set("output", output.to_owned());
        args.set("type", file_type.to_owned());
        args.set("bytes", bytes as i64);
        self.format_with("cli_encrypt_ok", &args)
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

struct LocaleEntry {
    lang: Language,
    display_name: &'static str,
    i18n: I18n,
}

static LOCALES: LazyLock<Vec<LocaleEntry>> = LazyLock::new(|| {
    let mut entries = Vec::new();
    let paths = crate::data::runtime_assets::list_runtime_assets("locales/", ".ftl");

    for path in paths {
        let Some(file_name) = path.rsplit('/').next() else {
            continue;
        };
        let Some(tag) = file_name.strip_suffix(".ftl") else {
            continue;
        };
        if tag.is_empty() {
            continue;
        }

        let source = crate::data::runtime_assets::read_runtime_asset_text(&path);
        let leaked_tag: &'static str = Box::leak(tag.to_string().into_boxed_str());
        let display = locale_display_name(leaked_tag, &source);

        entries.push(LocaleEntry {
            lang: Language { tag: leaked_tag },
            display_name: display,
            i18n: I18n::new(source, leaked_tag),
        });
    }

    if entries.is_empty() {
        panic!("No locale files found under runtime assets locales/*.ftl");
    }

    entries.sort_by(|a, b| a.lang.tag.cmp(b.lang.tag));
    entries
});

static ALL_LANGUAGES: LazyLock<Vec<Language>> =
    LazyLock::new(|| LOCALES.iter().map(|entry| entry.lang).collect());

fn locale_display_name(tag: &'static str, source: &str) -> &'static str {
    let name = source
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                return None;
            }

            let (key, value) = trimmed.split_once('=')?;
            (key.trim() == "locale_native_name").then(|| value.trim().to_string())
        })
        .filter(|name| !name.is_empty())
        .unwrap_or_else(|| tag.to_string());

    Box::leak(name.into_boxed_str())
}

fn locale_entry(lang: Language) -> Option<&'static LocaleEntry> {
    LOCALES.iter().find(|entry| entry.lang == lang)
}

fn english_language() -> Language {
    LOCALES
        .iter()
        .find(|entry| entry.lang.tag.eq_ignore_ascii_case("en-US"))
        .map(|entry| entry.lang)
        .or_else(|| {
            LOCALES
                .iter()
                .find(|entry| entry.lang.tag.eq_ignore_ascii_case("en"))
                .map(|entry| entry.lang)
        })
        .unwrap_or_else(|| LOCALES[0].lang)
}

impl Language {
    pub fn i18n(self) -> &'static I18n {
        locale_entry(self)
            .map(|entry| &entry.i18n)
            .unwrap_or_else(|| {
                &locale_entry(english_language())
                    .expect("missing locale")
                    .i18n
            })
    }

    /// Native name shown in the language picker (always in its own language).
    pub fn display_name(self) -> &'static str {
        locale_entry(self)
            .map(|entry| entry.display_name)
            .unwrap_or(self.tag)
    }

    /// All available languages, in menu order.
    pub fn all() -> &'static [Language] {
        ALL_LANGUAGES.as_slice()
    }

    #[cfg(test)]
    pub fn english() -> Self {
        english_language()
    }

    /// Detect language from the OS locale, falling back to English.
    pub fn from_system() -> Self {
        let tag = sys_locale::get_locale().unwrap_or_default();
        if tag.is_empty() {
            return english_language();
        }

        if let Some(found) = LOCALES
            .iter()
            .find(|entry| entry.lang.tag.eq_ignore_ascii_case(&tag))
            .map(|entry| entry.lang)
        {
            return found;
        }

        let primary = tag.split('-').next().unwrap_or_default();
        if let Some(found) = LOCALES
            .iter()
            .find(|entry| {
                entry.lang.tag.eq_ignore_ascii_case(primary)
                    || entry
                        .lang
                        .tag
                        .to_ascii_lowercase()
                        .starts_with(&(primary.to_ascii_lowercase() + "-"))
            })
            .map(|entry| entry.lang)
        {
            return found;
        }

        english_language()
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::{Path, PathBuf};

    fn project_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    fn locale_dir() -> PathBuf {
        project_root().join("assets/locales")
    }

    fn source_dir() -> PathBuf {
        project_root().join("src")
    }

    fn parse_ftl_keys(path: &Path) -> BTreeSet<String> {
        let source = fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        source
            .lines()
            .filter_map(|line| {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') {
                    return None;
                }

                let (key, _) = line.split_once('=')?;
                let key = key.trim();
                (!key.is_empty()).then(|| key.to_string())
            })
            .collect()
    }

    fn locale_files() -> Vec<PathBuf> {
        let mut files: Vec<_> = fs::read_dir(locale_dir())
            .expect("failed to read locale dir")
            .map(|entry| entry.expect("failed to read locale dir entry").path())
            .filter(|path| path.extension().is_some_and(|ext| ext == "ftl"))
            .collect();
        files.sort();
        files
    }

    fn collect_source_texts(dir: &Path, texts: &mut Vec<String>) {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
            .map(|entry| entry.expect("failed to read source dir entry").path())
            .collect();
        entries.sort();

        for path in entries {
            if path.is_dir() {
                collect_source_texts(&path, texts);
                continue;
            }

            if path.extension().is_some_and(|ext| ext == "rs") {
                texts.push(
                    fs::read_to_string(&path).unwrap_or_else(|error| {
                        panic!("failed to read {}: {error}", path.display())
                    }),
                );
            }
        }
    }

    fn used_locale_keys(keys: &BTreeSet<String>) -> BTreeSet<String> {
        let mut texts = Vec::new();
        collect_source_texts(&source_dir(), &mut texts);

        keys.iter()
            .filter(|key| {
                let needle = format!("\"{key}\"");
                texts.iter().any(|text| text.contains(&needle))
            })
            .cloned()
            .collect()
    }

    #[test]
    fn locale_key_sets_match_english_baseline() {
        let locale_files = locale_files();
        let english_path = locale_dir().join("en-US.ftl");
        let english_keys = parse_ftl_keys(&english_path);

        for path in locale_files {
            let keys = parse_ftl_keys(&path);
            assert_eq!(
                keys,
                english_keys,
                "locale key set mismatch for {}",
                path.display()
            );
        }
    }

    #[test]
    fn english_locale_has_no_unused_keys() {
        let english_path = locale_dir().join("en-US.ftl");
        let keys = parse_ftl_keys(&english_path);
        let used_keys = used_locale_keys(&keys);
        let unused: Vec<_> = keys.difference(&used_keys).cloned().collect();

        assert!(
            unused.is_empty(),
            "unused locale keys found in en-US.ftl: {}",
            unused.join(", ")
        );
    }
}
