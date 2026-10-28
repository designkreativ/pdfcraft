//! UI localisation. Strings in code stay English and are the default lookup keys; a per-language
//! catalog (`*.tsv`, see `ja.tsv` for the format) maps them to display text at render time. Command
//! ids, document text, file names, the control channel, the CLI and MCP never see translated text,
//! so agents and scripts are unaffected. A string without a translation is shown in English, so
//! coverage can grow incrementally.
//!
//! The system is the one PhotoCraft uses (`photocraft/crates/ui-egui/src/i18n`), so catalogs and
//! habits carry over between the two apps.
//!
//! # Adding a language
//! 1. Add `xx.tsv` next to `ja.tsv` (copy its header; translate from the *meaning* of the English
//!    text, clean-room, see `ja.tsv`).
//! 2. Add one row to [`LANGUAGES`] (code, native name, catalog, plural rule).
//!
//! That is all: the Preferences dropdown, the system-locale match and the catalog tests (parse,
//! placeholders, plural forms) pick it up from the registry.
//!
//! # Looking strings up
//! - [`tl!`](crate::tl) / [`t`]: a plain string in the current language. [`tr`]: in a given one.
//! - [`tr_ctx`] (`tl_ctx!` in UI code): when one English word needs different translations.
//! - [`tr_id`]: a command-id keyed string with the English label as fallback (menu items), so a
//!   translation survives rewording of the English text and can differ per command.
//! - [`trn`]: plural-aware (`{n}` is filled in). [`fmt`]: fill `{name}` placeholders after [`tr`];
//!   translators may reorder placeholders freely.

mod catalog;

use std::cell::Cell;
use std::sync::OnceLock;

use catalog::Catalog;

/// The `language` preference value that follows the system locale.
pub const AUTO: &str = "auto";

/// One supported UI language.
pub struct LangInfo {
    /// BCP 47 code, lowercase (`ja`, `zh-hans`, `pt-br`). Also the `language` preference value.
    pub code: &'static str,
    /// The language's name in itself, shown in the Preferences dropdown.
    pub name: &'static str,
    /// Catalog file contents (empty for the built-in English).
    pub source: &'static str,
    /// Plural form index for a count (English: 0 = one, 1 = other; Japanese: always 0). A catalog's
    /// `@plural` entries list one form per index.
    pub plural: fn(u64) -> usize,
    /// The language is written right to left. egui lays text out left to right, so the catalog's
    /// translations are put into display order when it loads ([`crate::bidi::display_rtl`]).
    pub rtl: bool,
    catalog: OnceLock<Catalog>,
}

fn plural_one_other(n: u64) -> usize {
    usize::from(n != 1)
}

fn plural_none(_: u64) -> usize {
    0
}

/// French (CLDR `fr`): 0 and 1 take the singular, everything else the plural.
fn plural_fr(n: u64) -> usize {
    usize::from(n > 1)
}

/// The registry. English first: it is the fallback and the source language.
pub static LANGUAGES: [LangInfo; 4] = [
    LangInfo { code: "en", name: "English", source: "", plural: plural_one_other, rtl: false, catalog: OnceLock::new() },
    // Spanish (European vocabulary); every `es-*` locale (`es-ES`, `es-MX`, `es-419` ...) resolves here.
    LangInfo { code: "es", name: "Español", source: include_str!("es.tsv"), plural: plural_one_other, rtl: false, catalog: OnceLock::new() },
    // French; every `fr-*` locale (`fr-FR`, `fr-CA`, `fr-BE` ...) resolves here.
    LangInfo { code: "fr", name: "Français", source: include_str!("fr.tsv"), plural: plural_fr, rtl: false, catalog: OnceLock::new() },
    // Turkish; every `tr-*` locale (`tr-TR`, `tr_TR.UTF-8`) resolves here.
    LangInfo { code: "tr", name: "Türkçe", source: include_str!("tr.tsv"), plural: plural_none, rtl: false, catalog: OnceLock::new() },
];

impl LangInfo {
    /// How many plural forms the language's `@plural` entries list.
    fn plural_forms(&self) -> usize {
        (0..=1000).map(self.plural).max().unwrap_or(0).saturating_add(1)
    }

    fn catalog(&self) -> &Catalog {
        self.catalog.get_or_init(|| {
            let (mut catalog, errors) = Catalog::parse(self.source, self.plural_forms());
            for error in errors {
                log::warn!("{} interface catalog: {error}; that entry shows in English", self.code);
            }
            if self.rtl {
                catalog.map_translations(crate::bidi::display_rtl);
            }
            catalog
        })
    }
}

/// A language the UI can be shown in (a handle into [`LANGUAGES`]).
#[derive(Clone, Copy)]
pub struct Lang(&'static LangInfo);

impl std::fmt::Debug for Lang {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Lang({})", self.0.code)
    }
}

impl PartialEq for Lang {
    fn eq(&self, other: &Self) -> bool {
        self.0.code == other.0.code
    }
}

impl Eq for Lang {}

impl Lang {
    pub const EN: Lang = Lang(&LANGUAGES[0]);

    pub fn code(self) -> &'static str {
        self.0.code
    }

    /// A language by its exact code.
    pub fn from_code(code: &str) -> Option<Lang> {
        LANGUAGES.iter().find(|l| l.code.eq_ignore_ascii_case(code)).map(Lang)
    }

    /// Resolve the `language` preference: a language code, or [`AUTO`] (and anything unknown,
    /// e.g. a code from a newer version) to follow the system locale.
    pub fn from_pref(pref: &str) -> Lang {
        Lang::from_code(pref).unwrap_or_else(system_lang)
    }

    /// Every registered language.
    pub fn all() -> impl Iterator<Item = Lang> {
        LANGUAGES.iter().map(Lang)
    }

    pub fn name(self) -> &'static str {
        self.0.name
    }

    /// Whether the language is written right to left. Its translations come back from the
    /// lookups in display order (see [`LangInfo::rtl`]).
    pub fn rtl(self) -> bool {
        self.0.rtl
    }

    fn catalog(self) -> &'static Catalog {
        self.0.catalog()
    }
}

/// The canonical `language` preference for user input: [`AUTO`] or a registered code (any case).
/// `None` for anything else, so callers can keep the current setting.
pub fn normalize_pref(pref: &str) -> Option<&'static str> {
    if pref.eq_ignore_ascii_case(AUTO) {
        return Some(AUTO);
    }
    Lang::from_code(pref).map(Lang::code)
}

/// Candidate language codes for a locale tag, most specific first: `zh_TW.UTF-8` →
/// `zh-tw`, `zh-hant`, `zh`.
fn candidates(tag: &str) -> Vec<String> {
    let base = tag.split(['.', '@']).next().unwrap_or("").replace('_', "-").to_ascii_lowercase();
    let parts: Vec<&str> = base.split('-').filter(|p| !p.is_empty()).collect();
    if parts.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for n in (1..=parts.len()).rev() {
        out.push(parts.get(..n).unwrap_or_default().join("-"));
    }
    out
}

/// The registered language for a locale tag such as `ja_JP.UTF-8`, `ja-JP`; `None` if the
/// language isn't supported. `C`/`POSIX` mean English.
pub fn lang_from_tag(tag: &str) -> Option<Lang> {
    let cands = candidates(tag);
    if matches!(cands.first().map(String::as_str), Some("c" | "posix")) {
        return Some(Lang::EN);
    }
    cands.iter().find_map(|c| Lang::from_code(c))
}

/// The system language (cached). English when it can't be determined.
pub fn system_lang() -> Lang {
    // Tests drive the UI by its English labels whatever the developer's locale is. This covers
    // the unit tests; the integration tests link the library as it ships, where `cfg!(test)` is
    // off, so each of their harnesses pins the `language` option to `en` instead.
    if cfg!(test) {
        return Lang::EN;
    }
    static SYSTEM: OnceLock<Lang> = OnceLock::new();
    *SYSTEM.get_or_init(detect_system_lang)
}

#[cfg(not(target_arch = "wasm32"))]
fn detect_system_lang() -> Lang {
    for var in ["LC_ALL", "LC_MESSAGES", "LANG"] {
        if let Some(l) = std::env::var(var).ok().and_then(|v| language_override(&v)) {
            return l;
        }
    }
    // Apps started from the Finder don't inherit LANG: use the macOS preferred-languages list.
    // The absolute path keeps a `defaults` earlier on PATH from running; any failure means English.
    #[cfg(target_os = "macos")]
    if let Ok(out) = std::process::Command::new("/usr/bin/defaults").args(["read", "-g", "AppleLanguages"]).output()
        && out.status.success()
        && let Some(l) = first_supported(&String::from_utf8_lossy(&out.stdout))
    {
        return l;
    }
    #[cfg(target_os = "windows")]
    if let Some(l) = windows_ui_language().as_deref().and_then(first_supported) {
        return l;
    }
    Lang::EN
}

/// A named locale can override the display language. On Windows, the neutral
/// C/POSIX locale inherited from a launcher says nothing about the user's UI language.
#[cfg(not(target_arch = "wasm32"))]
fn language_override(tag: &str) -> Option<Lang> {
    let tag = tag.trim();
    if cfg!(target_os = "windows") && matches!(candidates(tag).first().map(String::as_str), Some("c" | "posix")) {
        return None;
    }
    lang_from_tag(tag)
}

/// The Windows display languages in preference order, one tag per line, queried once when Auto
/// first resolves. `sys-locale` asks Windows directly (`GetUserPreferredUILanguages`), so no
/// process is started.
#[cfg(target_os = "windows")]
fn windows_ui_language() -> Option<String> {
    let tags: Vec<String> = sys_locale::get_locales().collect();
    (!tags.is_empty()).then(|| tags.join("\n"))
}

/// The first supported language in a macOS `defaults read` list or Windows UI-culture output.
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
fn first_supported(list: &str) -> Option<Lang> {
    list.split(['(', ')', ',', '"', '\n']).map(str::trim).filter(|s| !s.is_empty()).find_map(lang_from_tag)
}

/// The first supported language among BCP 47 tags in preference order, as the browser lists them
/// in `navigator.languages` (#2954). Only the first few tags are looked at: the list is short in
/// practice, and a page can't make Auto walk an unbounded one.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
fn first_supported_tag<S: AsRef<str>>(tags: impl IntoIterator<Item = S>) -> Option<Lang> {
    tags.into_iter().take(32).find_map(|tag| lang_from_tag(tag.as_ref().trim()))
}

/// The browser's preferred languages, as the desktop builds use the system's: `navigator.languages`
/// in order, then `navigator.language` (kept within the 32 tags `first_supported_tag` reads).
/// English when the browser offers none we have a catalog for.
#[cfg(target_arch = "wasm32")]
fn detect_system_lang() -> Lang {
    let Some(navigator) = web_sys::window().map(|w| w.navigator()) else { return Lang::EN };
    let mut tags: Vec<String> = navigator.languages().iter().take(31).filter_map(|v| v.as_string()).collect();
    tags.extend(navigator.language());
    first_supported_tag(&tags).unwrap_or(Lang::EN)
}

thread_local! {
    // Per thread, as in PhotoCraft: independent app and test threads (kittest harnesses run in
    // parallel) must not change each other's drawing language.
    static CURRENT: Cell<Lang> = const { Cell::new(Lang::EN) };
}

/// Set the UI language for drawing (the shell calls this once per frame from the preference), so
/// widgets can translate without every call site carrying a language around.
pub fn set_current(lang: Lang) {
    CURRENT.set(lang);
}

/// The language the UI is drawn in.
/// `label` as it reads inside a sentence ("add a text box"): lowercased (Turkish with its dotted
/// and dotless i).
pub fn in_sentence(label: &str) -> std::borrow::Cow<'_, str> {
    match current().code() {
        "tr" => std::borrow::Cow::Owned(label.chars().flat_map(lower_turkish).collect()),
        _ => std::borrow::Cow::Owned(label.to_lowercase()),
    }
}

/// The translation of `english` in upper case, for section titles. Rust's case mapping knows no
/// language, and Turkish has a dotted and a dotless i (`i` ↔ `İ`, `ı` ↔ `I`): "Gelişmiş" must
/// become "GELİŞMİŞ", not "GELIŞMIŞ". A label without a Turkish row is still English and keeps
/// the English rule.
pub fn upper_label(english: &str) -> String {
    let lang = current();
    let shown = tr(lang, english);
    if lang.code() == "tr" && has(lang, english) { shown.chars().flat_map(upper_turkish).collect() } else { shown.to_uppercase() }
}

fn upper_turkish(c: char) -> std::char::ToUppercase {
    match c {
        'i' => 'İ'.to_uppercase(),
        'ı' => 'I'.to_uppercase(),
        c => c.to_uppercase(),
    }
}

fn lower_turkish(c: char) -> std::char::ToLowercase {
    match c {
        // `İ`.to_lowercase() is "i" plus a combining dot above.
        'İ' => 'i'.to_lowercase(),
        'I' => 'ı'.to_lowercase(),
        c => c.to_lowercase(),
    }
}

pub fn current() -> Lang {
    CURRENT.get()
}

/// Does `lang` have a catalog entry for this plain string? (English never does: it is the source.)
pub fn has(lang: Lang, s: &str) -> bool {
    lang.catalog().plain(s).is_some()
}

/// Translate an English UI string into the current language ([`tr`] with [`current`]).
pub fn t(s: &str) -> &str {
    tr(current(), s)
}

/// The name of a keyboard key, distinct from a command such as Delete. Missing key-name
/// translations use the English key name, never a plain command translation.
pub fn key_name(s: &str) -> &str {
    current().catalog().contextual("key", s).unwrap_or(s)
}

/// Translate an English UI string; unknown strings come back unchanged.
pub fn tr(lang: Lang, s: &str) -> &str {
    lang.catalog().plain(s).unwrap_or(s)
}

/// How the layers below say their own sentences in the current language.
///
/// The checker, the preflight profiles and the signature panel hand over a whole English wording
/// — holes and all — and the document's own words apart from it, so this is all they need of us:
/// `f.said(&i18n::words)` says the sentence here, and `in_english` says it in English. An unknown
/// wording comes back as it stands, which is the English, so a catalog that is missing one is a
/// sentence in English rather than a gap.
pub fn words(wording: &str) -> String {
    t(wording).to_owned()
}

/// Like [`tr`], for an English string that needs a disambiguating `context`.
pub fn tr_ctx<'a>(lang: Lang, context: &str, s: &'a str) -> &'a str {
    lang.catalog().contextual(context, s).unwrap_or_else(|| tr(lang, s))
}

/// A string keyed by its command id, falling back to the translation of the English `label`.
pub fn tr_id<'a>(lang: Lang, id: &str, label: &'a str) -> &'a str {
    lang.catalog().id(id).unwrap_or_else(|| tr(lang, label))
}

/// Fill `{name}` placeholders in one pass. Unknown placeholders are left as written, and inserted
/// values are never read as templates again, so a file name containing `{n}` stays intact.
pub fn fmt(template: &str, args: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some((before, after_open)) = rest.split_once('{') {
        out.push_str(before);
        let Some((name, after_close)) = after_open.split_once('}') else {
            out.push('{');
            out.push_str(after_open);
            return out;
        };
        match args.iter().find(|(key, _)| *key == name) {
            Some((_, value)) => out.push_str(value),
            None => {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
        }
        rest = after_close;
    }
    out.push_str(rest);
    out
}

/// A generated history label (the action part of "Undo …") in the current language. Captured
/// values (file and field names) are inserted as they are; unknown labels are looked up whole.
pub fn action_label(text: &str) -> String {
    for (prefix, template) in [
        ("Insert pages from ", "Insert pages from {name}"),
        ("Fill in ", "Fill in {name}"),
        ("Set the image of ", "Set the image of {name}"),
        ("Edit script of ", "Edit script of {name}"),
        ("Import ", "Import {name}"),
    ] {
        if let Some(value) = text.strip_prefix(prefix) {
            return fmt(t(template), &[("name", value)]);
        }
    }
    if let Some(key) = text.strip_prefix("Change ")
        && ["Title", "Author", "Subject", "Keywords", "Creator", "Producer"].contains(&key)
    {
        return fmt(t("Change {key}"), &[("key", t(key))]);
    }
    if let Some(n) = text.strip_prefix("Delete ").and_then(|rest| rest.strip_suffix(" fields")).and_then(|n| n.parse::<u64>().ok()) {
        return trn(current(), n, "Delete {n} field", "Delete {n} fields");
    }
    t(text).to_owned()
}

/// A history label in the current language, out of the engine's own two parts: the wording is
/// looked up whole and the document's own word goes into its `{}` afterwards, so a file or field
/// name is never read as part of the wording. This is exact where [`action_label`] has to guess
/// from the English text, and is what the engine's own labels go through.
pub fn edit_label(label: &pdfcraft_engine::Label) -> String {
    // A batch's wording is only the hole: it carries its name as text, which from below L7 is one
    // of `BATCH_LABELS` and so is looked up too.
    if let Some(name) = label.value.as_deref().filter(|_| label.wording == "{batch}") {
        return t(name).to_owned();
    }
    label.said(t(label.wording))
}

/// A command label such as "Undo Insert pages from a.pdf" in the current language. A catalog
/// whose word order doesn't put the verb first translates "Undo {action}" and "Redo {action}"
/// as whole phrases; otherwise the translated verb goes before the action.
pub fn command_label(text: &str) -> String {
    for (prefix, template) in [("Undo", "Undo {action}"), ("Redo", "Redo {action}")] {
        if let Some(action) = text.strip_prefix(prefix).and_then(|tail| tail.strip_prefix(' ')) {
            let action = action_label(action);
            if has(current(), template) {
                return fmt(t(template), &[("action", &action)]);
            }
            return format!("{} {action}", t(prefix));
        }
    }
    t(text).to_owned()
}

/// A registered command's menu label: its `@id` catalog entry if there is one, otherwise
/// [`command_label`] of the current English label.
pub fn menu_label(id: &str, label: &str) -> String {
    match current().catalog().id(id) {
        Some(translated) => translated.to_owned(),
        None => command_label(label),
    }
}

/// A plural-aware message: `one`/`other` are the English forms (with `{n}` where the count goes).
pub fn trn(lang: Lang, n: u64, one: &str, other: &str) -> String {
    let idx = (lang.0.plural)(n);
    let text = lang.catalog().plural(one, other, idx).unwrap_or(if n == 1 { one } else { other });
    fmt(text, &[("n", &n.to_string())])
}

#[cfg(test)]
mod tests {
    use super::catalog::{Entry, parse_entries, placeholders};
    use super::*;

    const FR: fn() -> Lang = || Lang::from_code("fr").expect("fr registered");
    // These physical key-cap names remain English in several keyboard layouts. Only their
    // dedicated key context may use them unchanged; ordinary command labels still translate.
    const KEY_CAP_NAMES: &[&str] = &["Ctrl", "Shift", "Delete", "Esc", "Home", "End"];

    #[test]
    fn keyboard_key_names_have_a_separate_context_and_english_fallback() {
        for lang in Lang::all().filter(|l| *l != Lang::EN) {
            for key in ["Ctrl", "Shift", "Delete", "Esc", "Home", "End", "Space"] {
                assert!(lang.catalog().contextual("key", key).is_some(), "{}: {key}", lang.code());
            }
        }
        let previous = current();
        set_current(Lang::from_code("fr").unwrap());
        assert_eq!(key_name("Delete"), "Suppr");
        assert_eq!(t("Delete"), "Supprimer");
        assert_eq!(key_name("Open"), "Open", "a missing key name must not borrow a command translation");
        set_current(Lang::EN);
        assert_eq!(key_name("Delete"), "Delete");
        set_current(previous);
    }

    #[test]
    fn tags_map_to_languages() {
        assert_eq!(lang_from_tag("tr_TR.UTF-8"), Lang::from_code("tr"));
        assert_eq!(lang_from_tag("es-419"), Lang::from_code("es"));
        assert_eq!(lang_from_tag("en_US.UTF-8"), Some(Lang::EN));
        assert_eq!(lang_from_tag("C"), Some(Lang::EN));
        assert_eq!(lang_from_tag("POSIX"), Some(Lang::EN));
        assert_eq!(lang_from_tag("fr_FR"), Lang::from_code("fr"));
        assert_eq!(lang_from_tag("fr_CA.UTF-8"), Lang::from_code("fr"));
        assert_eq!(lang_from_tag("fr-BE"), Lang::from_code("fr"));
        assert_eq!(lang_from_tag("ja-JP"), None, "no catalog for it");
        assert_eq!(lang_from_tag(""), None);
        assert_eq!(lang_from_tag("_"), None);
    }
    #[test]
    fn candidates_walk_from_specific_to_general() {
        assert_eq!(candidates("fr_CA.UTF-8"), ["fr-ca", "fr"]);
        assert_eq!(candidates("es-419"), ["es-419", "es"]);
        assert_eq!(candidates("tr"), ["tr"]);
        assert!(candidates("_").is_empty());
    }
    #[test]
    fn macos_language_list_is_parsed() {
        assert_eq!(first_supported("(\n    \"ja-JP\",\n    \"tr-TR\"\n)\n"), Lang::from_code("tr"), "skips a language without a catalog");
        assert_eq!(first_supported("(\n    \"fr-FR\",\n    \"en-US\"\n)\n"), Lang::from_code("fr"));
        assert_eq!(first_supported("("), None);
    }

    /// #2954: the web build's Auto language follows the browser's preferred languages in order,
    /// skipping ones without a catalog, and is English only when none is supported.
    #[test]
    fn browser_language_list_is_followed_in_order() {
        assert_eq!(first_supported_tag(["es-ES", "en-US"]), Lang::from_code("es"));
        assert_eq!(first_supported_tag(["xx-YY", "ja", "fr-FR"]), Some(FR()));
        assert_eq!(first_supported_tag(["zh-TW"]), None);
        assert_eq!(first_supported_tag(["en-GB", "fr-FR"]), Some(Lang::EN));
        assert_eq!(first_supported_tag([" tr-TR "]), Lang::from_code("tr"));
        assert_eq!(first_supported_tag(["", "xx"]), None);
        assert_eq!(first_supported_tag(Vec::<String>::new()), None);
        let long = std::iter::repeat_n("xx", 10_000).chain(["fr"]);
        assert_eq!(first_supported_tag(long), None, "only the first tags are read");
    }

    #[test]
    fn windows_display_language_resolves_chinese_scripts() {
        for tag in ["zh-CN", "zh-SG", "zh-Hans", "zh-Hans-CN"] {
            assert_eq!(first_supported(&format!("{tag}\r\n")), Lang::from_code("zh-hans"));
        }
        for tag in ["zh-TW", "zh-HK", "zh-MO", "zh-Hant", "zh-Hant-HK"] {
            assert_eq!(first_supported(&format!("{tag}\r\n")), Lang::from_code("zh-hant"));
        }
        assert_eq!(first_supported("en-US\r\n"), Some(Lang::EN));
        assert_eq!(first_supported("fr-FR\r\n"), Lang::from_code("fr"));
        assert_eq!(first_supported("\r\n"), None);
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn neutral_locale_does_not_override_display_language() {
        let fr = Lang::from_code("fr").unwrap();
        for tag in ["C", "C.UTF-8", "POSIX", "posix.UTF-8", "", "  C.UTF-8  ", "unsupported"] {
            assert_eq!(language_override(tag), None, "{tag}");
            assert_eq!(language_override(tag).or_else(|| first_supported("unsupported-XY\nfr-FR\nen-US")), Some(fr));
        }
        assert_eq!(["C.UTF-8", "POSIX", "fr_FR.UTF-8"].into_iter().find_map(language_override), Some(fr));
        assert_eq!(language_override("fr_FR.UTF-8"), Some(fr));
        assert_eq!(language_override("en_US.UTF-8"), Some(Lang::EN));
        assert_eq!(language_override("es_ES.UTF-8"), Lang::from_code("es"));
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_display_language_query_returns_a_locale_tag() {
        // One tag per line, as `first_supported` reads them: a machine with several display
        // languages reports them all, so each line is checked rather than the list as a whole.
        let list = windows_ui_language().expect("Windows should report a display language");
        let tags: Vec<&str> = list.lines().map(str::trim).filter(|t| !t.is_empty()).collect();
        assert!(!tags.is_empty(), "no display language in {list:?}");
        for tag in tags {
            assert!(tag.split('-').all(|part| !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric())), "not a locale tag: {tag:?}");
        }
    }

    #[test]
    fn preferences_resolve_with_fallback() {
        assert_eq!(Lang::from_pref("fr"), FR());
        assert_eq!(Lang::from_pref("FR"), FR());
        assert_eq!(Lang::from_pref("en"), Lang::EN);
        // `auto` and unknown codes follow the system (English under test).
        assert_eq!(Lang::from_pref(AUTO), Lang::EN);
        assert_eq!(Lang::from_pref("xx-unknown"), Lang::EN);
        assert_eq!(normalize_pref("FR"), Some("fr"));
        assert_eq!(normalize_pref("Auto"), Some(AUTO));
        assert_eq!(normalize_pref("xx"), None);
    }

    #[test]
    fn lookups_fall_back_to_english() {
        assert_eq!(tr(FR(), "no such label"), "no such label");
        assert_eq!(tr(Lang::EN, "File"), "File");
        assert_eq!(tr(FR(), "File"), "Fichier");
        assert_eq!(tr(FR(), "日本語の文書.pdf"), "日本語の文書.pdf");
        assert_eq!(tr_id(FR(), "no.such.id", "File"), "Fichier");
        assert_eq!(tr_ctx(FR(), "no such context", "File"), "Fichier");
        assert!(has(FR(), "Save") && !has(Lang::EN, "Save"));
    }

    #[test]
    fn catalog_kinds_are_parsed_and_looked_up() {
        let (c, errors) = Catalog::parse(
            "# c\n\tHello\tこんにちは\n@id\tfile.save\t保存する\nmenu\tWindows\tウィンドウ群\n@plural\t{n} file|{n} files\t{n} 個\n\n",
            1,
        );
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(c.plain("Hello"), Some("こんにちは"));
        assert_eq!(c.id("file.save"), Some("保存する"));
        assert_eq!(c.contextual("menu", "Windows"), Some("ウィンドウ群"));
        assert_eq!(c.contextual("other", "Windows"), None);
        assert_eq!(c.plural("{n} file", "{n} files", 0), Some("{n} 個"));
        assert_eq!(c.plural("{n} file", "{n} files", 5), Some("{n} 個"), "an index past the forms clamps");
    }

    #[test]
    fn malformed_lines_are_reported_not_fatal() {
        let (entries, errors) = parse_entries("\tok\tはい\nno tabs here\n\tonly\n\ta\tb\tc\textra\n\t\tempty source\n", 1);
        assert_eq!(entries.len(), 1);
        assert_eq!(errors.len(), 4, "{errors:?}");
        let (entries, errors) = parse_entries("\ta\\tb\tx\\ny\\\\z\n", 1);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(entries, [Entry { context: String::new(), source: "a\tb".into(), translation: "x\ny\\z".into() }]);
    }

    #[test]
    fn strict_validation_skips_only_the_bad_line() {
        for (text, forms, error) in [
            ("\tFile", 1, "context<TAB>"),
            ("\tFile\t", 1, "nonempty"),
            ("\tFile\\q\tFiles", 1, "unknown escape"),
            ("\tFile\tFiles\\", 1, "trailing backslash"),
            ("\tOpen…\tOpen", 1, "ellipsis"),
            ("\tOpen {file}\tOpen {name}", 1, "placeholders"),
            ("@plural\tpage\tPages", 1, "one|other"),
            ("@plural\t{n} page|{n} pages\t{n} pages", 2, "2 nonempty"),
            ("@plural\t{n} page|{n} pages\tPages", 1, "placeholders"),
            ("@unknown\tFile\tFiles", 1, "reserved context"),
        ] {
            // The bad line comes first; the good line after it still loads.
            let (catalog, errors) = Catalog::parse(&format!("{text}\n\tSave\t保存\n"), forms);
            assert_eq!(errors.len(), 1, "{text:?}: {errors:?}");
            assert!(errors[0].starts_with("line 1: ") && errors[0].contains(error), "{errors:?}");
            assert_eq!(catalog.plain("Save"), Some("保存"), "{text:?}");
        }
        let (catalog, errors) = Catalog::parse("\tFile\tファイル\n\tFile\t別\n", 1);
        assert!(errors.len() == 1 && errors[0].contains("line 2: duplicate"), "{errors:?}");
        assert_eq!(catalog.plain("File"), Some("ファイル"), "the first entry wins");
    }

    #[test]
    fn catalogs_handle_escapes_crlf_and_distinct_contexts() {
        let (catalog, errors) =
            Catalog::parse("# comment\r\n\r\n\tLine\\nTab\\tPath\\\\\tOther\\nTab\\tPath\\\\\r\nweight\tLight\tThin\r\ntheme\tLight\tPale\r\n", 1);
        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(catalog.plain("Line\nTab\tPath\\"), Some("Other\nTab\tPath\\"));
        assert_eq!(catalog.contextual("weight", "Light"), Some("Thin"));
        assert_eq!(catalog.contextual("theme", "Light"), Some("Pale"));
    }

    #[test]
    fn broken_catalog_falls_back_to_english() {
        static BROKEN: LangInfo = LangInfo {
            code: "broken",
            name: "Broken test catalog",
            source: "malformed\n\\",
            plural: plural_none,
            rtl: false,
            catalog: OnceLock::new(),
        };
        assert_eq!(tr(Lang(&BROKEN), "File"), "File");
        assert_eq!(trn(Lang(&BROKEN), 2, "{n} page", "{n} pages"), "2 pages");
    }

    #[test]
    fn contextual_id_and_plural_lookups_have_english_fallbacks() {
        static TEST: LangInfo = LangInfo {
            code: "test",
            name: "Test catalog",
            source: "\tLight\tPlain light\nweight\tLight\tThin\n@id\tfile.open\tOpen dialog…\n@plural\t{n} page|{n} pages\tOne page: {n}|Many pages: {n}\n",
            plural: plural_one_other,
            rtl: false,
            catalog: OnceLock::new(),
        };
        let l = Lang(&TEST);
        assert_eq!(tr_ctx(l, "weight", "Light"), "Thin");
        assert_eq!(tr_ctx(l, "theme", "Light"), "Plain light");
        assert_eq!(tr_ctx(l, "theme", "Unknown"), "Unknown");
        assert_eq!(tr_id(l, "file.open", "Reworded English…"), "Open dialog…");
        assert_eq!(tr_id(l, "unknown", "Light"), "Plain light");
        assert_eq!(tr_id(l, "unknown", "Unknown"), "Unknown");
        assert_eq!(trn(l, 1, "{n} page", "{n} pages"), "One page: 1");
        assert_eq!(trn(l, 0, "{n} page", "{n} pages"), "Many pages: 0");
        assert_eq!(trn(l, u64::MAX, "{n} page", "{n} pages"), format!("Many pages: {}", u64::MAX));
    }

    #[test]
    fn formatting_reorders_parameters_without_reinterpreting_user_text() {
        let args = [("file", "日本語-{n}.pdf"), ("n", "2")];
        assert_eq!(fmt("{n}: {file}; {unknown}", &args), "2: 日本語-{n}.pdf; {unknown}");
        assert_eq!(fmt("{file} / {file}", &args), "日本語-{n}.pdf / 日本語-{n}.pdf");
        assert_eq!(fmt("text {unfinished", &args), "text {unfinished");
        assert_eq!(fmt("}{n}{", &args), "}2{");
        assert_eq!(fmt("unchanged", &[]), "unchanged");
    }

    /// #103: the missing-OCR-models messages are translated under their current English text
    /// (catalogs once keyed an older `PRINTCRAFT_MODELS` wording, which never matched).
    #[test]
    fn missing_ocr_models_messages_are_translated() {
        let error = pdfcraft_engine::ocr::OcrError::NoModels.to_string();
        let notice = "Text recognition isn't installed: its model files are missing. Reinstall KreativPdf, or set PDFCRAFT_MODELS to the folder that holds them.";
        assert!(include_str!("../ocr_ui.rs").contains(notice), "keep in step with ocr_ui.rs");
        for code in ["es", "fr", "tr"] {
            let lang = Lang::from_code(code).expect("registered");
            assert!(has(lang, notice) && has(lang, &error), "{code}");
        }
        for lang in &LANGUAGES {
            assert!(!lang.source.contains("PRINTCRAFT"), "{}", lang.code);
        }
    }

    /// Every `tl!("…")` literal in the UI source (test modules aside), unescaped.
    /// Every label Shared Field Properties shows, including those it passes through a variable
    /// (flag labels, errors, the font label), exists in each full catalog.
    #[test]
    fn bulk_field_properties_labels_are_in_every_full_catalog() {
        let source = include_str!("../bulk_fields.rs");
        // The string literal starting right after `at`, unescaped.
        fn literal(rest: &str) -> Option<String> {
            let mut out = String::new();
            let mut chars = rest.chars();
            while let Some(c) = chars.next() {
                match c {
                    '"' => return Some(out),
                    '\\' => out.push(chars.next()?),
                    c => out.push(c),
                }
            }
            None
        }
        let mut labels = std::collections::BTreeSet::new();
        for marker in ["tl!(\"", "Err(\"", "ok_or(\"", "|| \"", "MISSING: &str = \"", "i18n::t(\"", "_ => \""] {
            for (at, _) in source.match_indices(marker) {
                labels.extend(source.get(at + marker.len()..).and_then(literal));
            }
        }
        // Flag labels: `(field_flags::NAME, "Label", inverted)`.
        for (at, _) in source.match_indices("(field_flags::") {
            let line = source.get(at..).and_then(|s| s.lines().next()).unwrap_or_default();
            if let Some(q) = line.find(", \"") {
                labels.extend(line.get(q + 3..).and_then(literal));
            }
        }
        labels.retain(|l| l.chars().any(char::is_alphabetic) && !l.contains("credits") && !l.starts_with("bulk"));
        for label in ["Mixed", "Custom font", "The document is no longer open."] {
            labels.insert(label.to_string());
        }
        assert!(labels.len() > 20 && labels.contains("Comb of characters"), "the scan found the dialog's labels: {labels:?}");
        for code in ["es", "fr", "tr"] {
            let lang = Lang::from_code(code).expect("registered");
            let missing: Vec<_> = labels.iter().filter(|l| !has(lang, l)).collect();
            assert!(missing.is_empty(), "{code} lacks Shared Field Properties labels: {missing:#?}");
        }
    }

    fn ui_literals() -> std::collections::BTreeSet<String> {
        scan_ui_literals(false)
    }

    // Lithuanian also checks direct lookup calls and multiline literal arguments.
    fn scan_ui_literals(expanded: bool) -> std::collections::BTreeSet<String> {
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut literals = std::collections::BTreeSet::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).expect("UI source directory") {
                let path = entry.expect("UI source entry").path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name != "i18n") {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let source = std::fs::read_to_string(path).expect("UI source file").replace("\r\n", "\n");
                    let source = if expanded { source.replace("crate::i18n::t(", "tl!(") } else { source };
                    let mut rest = source.split("#[cfg(test)]\nmod ").next().unwrap_or_default();
                    while let Some((_, after)) = rest.split_once(if expanded { "tl!(" } else { "tl!(\"" }) {
                        let after = if expanded {
                            let after = after.trim_start();
                            let Some(after) = after.strip_prefix('"') else {
                                rest = after;
                                continue;
                            };
                            after
                        } else {
                            after
                        };
                        let mut escaped = false;
                        let end = after
                            .char_indices()
                            .find_map(|(i, c)| {
                                if c == '"' && !escaped {
                                    return Some(i);
                                }
                                escaped = c == '\\' && !escaped;
                                None
                            })
                            .expect("closed tl! literal");
                        let (raw, tail) = after.split_at(end);
                        let argument_end = tail.strip_prefix('"').expect("closing quote").trim_start();
                        let literal_call =
                            argument_end.starts_with(')') || argument_end.strip_prefix(',').is_some_and(|rest| rest.trim_start().starts_with(')'));
                        if tail.starts_with("\")") || (expanded && literal_call) {
                            let label: String = serde_json::from_str(&format!("\"{raw}\"")).expect("UI literal escapes");
                            literals.insert(label);
                        }
                        rest = tail.strip_prefix('"').expect("closing quote");
                    }
                }
            }
        }
        literals
    }

    /// Partial catalogs also translate every label of the absolute local GoToR consent prompt.
    #[test]
    fn local_gotor_confirmation_is_translated_in_all_supported_languages() {
        for language in LANGUAGES.iter().filter(|language| !language.source.is_empty()) {
            let lang = Lang::from_code(language.code).expect("registered language");
            for label in [
                "Open this local PDF?",
                "This document is trying to open a local file:",
                "Only allow this if you trust the source document.",
                "Allow",
                "Cancel",
            ] {
                assert!(has(lang, label), "{} lacks GoToR confirmation label: {label}", language.code);
                assert_ne!(tr(lang, label), label, "{} falls back to English for {label}", language.code);
            }
        }
    }

    /// Every catalog says the labels of a visible signature, which are looked up by the sign
    /// crate's own English words (so the `tl!` scan can't see them), and the note shown when they
    /// fall back to English. Each label that runs into its value keeps a separator at its end.
    #[test]
    fn signature_appearance_labels_are_in_every_catalog() {
        let english = pdfcraft_engine::sign::AppearanceLabels::default();
        let note = "The labels are in English: the signature's font can't draw them in this language.";
        for language in LANGUAGES.iter().filter(|language| !language.source.is_empty()) {
            let lang = Lang::from_code(language.code).expect("registered language");
            assert!(has(lang, note), "{} lacks the labels note", language.code);
            for (field, label) in english.fields() {
                assert!(has(lang, label), "{} lacks the signature label {field}: {label:?}", language.code);
                let said = tr(lang, label);
                if field != "signed_by" && !lang.rtl() {
                    assert!(said.ends_with(' ') || said.ends_with('：'), "{}: the {field} label runs into its value: {said:?}", language.code);
                }
            }
        }
    }

    #[test]
    fn history_labels_keep_captured_names() {
        set_current(FR());
        assert_eq!(action_label("Insert pages from Save {e}.pdf"), "Insérer des pages depuis Save {e}.pdf");
        assert_eq!(command_label("Undo Insert pages from Save {e}.pdf"), "Annuler Insérer des pages depuis Save {e}.pdf");
        assert_eq!(action_label("Untranslated custom action"), "Untranslated custom action");
        assert_eq!(menu_label("file.open", "Open…"), "Ouvrir…");
        set_current(Lang::EN);
        assert_eq!(command_label("Undo Fill in Save {e}"), "Undo Fill in Save {e}");
    }

    #[test]
    fn current_language_is_per_thread() {
        set_current(FR());
        assert_eq!(t("File"), "Fichier");
        std::thread::spawn(|| assert_eq!(current(), Lang::EN)).join().unwrap();
        set_current(Lang::EN);
        assert_eq!(t("File"), "File");
    }

    #[test]
    fn plurals_and_placeholders() {
        assert_eq!(trn(Lang::EN, 1, "{n} item", "{n} items"), "1 item");
        assert_eq!(trn(Lang::EN, 0, "{n} item", "{n} items"), "0 items");
        assert_eq!(trn(Lang::EN, 7, "{n} item", "{n} items"), "7 items");
        // No `@plural` entry for it: a language falls back to the English forms.
        assert_eq!(trn(FR(), 7, "{n} item", "{n} items"), "7 items");
        assert_eq!(fmt("{b} before {a}", &[("a", "x"), ("b", "y"), ("c", "z")]), "y before x");
        assert_eq!(fmt("{missing}", &[]), "{missing}");
        assert_eq!(placeholders("a {x} b {y} {"), ["x", "y"]);
    }

    #[test]
    fn language_persists_and_invalid_input_keeps_current_language() {
        let mut app = crate::PdfCraftApp::default();
        assert_eq!(app.language, AUTO);
        app.set_option("language", "FR").unwrap();
        assert_eq!(app.language, "fr");
        assert!(app.set_option("language", "xx").is_err());
        assert_eq!(app.language, "fr");
        let mut restored = crate::PdfCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, "fr");
        restored.restore(r#"{"language":"xx"}"#);
        assert_eq!(restored.language, "fr");
        app.set_option("language", AUTO).unwrap();
        assert_eq!(app.language, AUTO);
        // Settings written before the system-locale default (no key) follow the system.
        let mut legacy = crate::PdfCraftApp::default();
        legacy.restore("{}");
        assert_eq!(legacy.language, AUTO);
    }

    #[test]
    fn spanish_is_registered() {
        let es = Lang::from_code("es").expect("es registered");
        assert_eq!(es.name(), "Español");
        assert_eq!(normalize_pref("ES"), Some("es"));
        assert_eq!(lang_from_tag("es_ES.UTF-8"), Some(es));
        assert_eq!(lang_from_tag("es-MX"), Some(es));
        assert_eq!(first_supported("es-ES\r\nen-US"), Some(es));
        assert_eq!(tr(es, "File"), "Archivo");
        assert_eq!(tr(es, "Save as…"), "Guardar como…");
        assert_eq!(tr(es, "Informe del usuario.pdf"), "Informe del usuario.pdf");
        assert_eq!(trn(es, 1, "{n} page", "{n} pages"), "1 página");
        assert_eq!(trn(es, 3, "{n} page", "{n} pages"), "3 páginas");
        // The signature pad's Type is the verb (type your name), not the noun "Tipo".
        assert_eq!(tr_ctx(es, "signature pad", "Type"), "Escribir");
        let mut app = crate::PdfCraftApp::default();
        app.set_option("language", "es").unwrap();
        assert_eq!(app.language, "es");
        let mut restored = crate::PdfCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, "es");
    }

    /// Spanish translates every registered command and every All tools group, section and item.
    #[test]
    fn spanish_covers_commands_and_catalogue() {
        let es = Lang::from_code("es").expect("es registered");
        for command in pdfcraft_engine::commands::COMMANDS {
            assert!(has(es, command.label), "missing command: {}", command.label);
            if let Some(menu) = command.menu {
                assert!(has(es, menu), "missing menu: {menu}");
            }
        }
        for group in pdfcraft_engine::catalog::TOOL_GROUPS {
            assert!(has(es, group.label), "missing group: {}", group.label);
            for section in group.sections {
                assert!(has(es, section.title), "missing section: {}", section.title);
                for item in section.items {
                    assert!(has(es, item.label), "missing item: {}", item.label);
                }
            }
        }
    }

    /// New tl!("literal") labels must not silently fall back to English in Spanish.
    #[test]
    fn spanish_covers_ui_literals() {
        let es = Lang::from_code("es").expect("es registered");
        let literals = ui_literals();
        assert!(literals.len() > 900, "source scan found only {} literals", literals.len());
        let missing: Vec<_> = literals.iter().filter(|label| !has(es, label)).collect();
        assert!(missing.is_empty(), "untranslated Spanish UI literals: {missing:#?}");
    }

    /// Spanish says everything the layers below word for themselves: what the accessibility
    /// checker finds and what its saved report calls things, what the PDF/A verifier reports, what
    /// a signature's details say and the changes they name, what can be wrong with the pages to
    /// print, and what an edit is called in the history.
    ///
    /// These lists are the whole point of the engine handing the wording over apart from the
    /// document's own words: a catalog that covers them says those sentences in its own language.
    /// A sentence missing here is a sentence in English on a Spanish screen.
    #[test]
    fn spanish_covers_what_the_engine_says() {
        let es = Lang::from_code("es").expect("es registered");
        let lists: [(&str, &[&str]); 8] = [
            ("accessibility finding", pdfcraft_engine::a11y::WORDINGS),
            ("accessibility report", pdfcraft_engine::a11y::REPORT_WORDS),
            ("PDF/A problem", pdfcraft_engine::pdfa::WORDINGS),
            ("signature detail", pdfcraft_engine::sign::pdf::WORDINGS),
            ("change after signing", pdfcraft_engine::sign::pdf::kind::ALL),
            ("pages to print", pdfcraft_engine::print::WORDINGS),
            ("edit label", pdfcraft_engine::EDIT_LABELS),
            ("batch label", pdfcraft_engine::BATCH_LABELS),
        ];
        for (what, list) in lists {
            assert!(!list.is_empty(), "no {what} wordings to cover");
            for wording in list {
                assert!(has(es, wording), "missing {what}: {wording}");
            }
        }
    }

    /// Each of those sentences keeps its holes when it is said in Spanish, so the document's own
    /// words still have somewhere to go: a translation that drops `{p}` or `{}` would read as a
    /// sentence about nothing in particular.
    #[test]
    fn spanish_keeps_the_holes_in_what_the_engine_says() {
        let es = Lang::from_code("es").expect("es registered");
        let lists: [&[&str]; 4] = [
            pdfcraft_engine::a11y::WORDINGS,
            pdfcraft_engine::pdfa::WORDINGS,
            pdfcraft_engine::sign::pdf::WORDINGS,
            pdfcraft_engine::print::WORDINGS,
        ];
        for wording in lists.into_iter().flatten() {
            let said = tr(es, wording);
            // `{}` is one hole; `{name}` holes are named, and each name must survive by itself.
            assert_eq!(wording.matches("{}").count(), said.matches("{}").count(), "the Spanish of {wording:?} loses a hole: {said:?}");
            for hole in wording.split('{').skip(1).filter_map(|rest| rest.split_once('}')).map(|(name, _)| name).filter(|n| !n.is_empty()) {
                assert!(said.contains(&format!("{{{hole}}}")), "the Spanish of {wording:?} loses {{{hole}}}: {said:?}");
            }
        }
    }

    /// Telugu puts the verb last, so it translates "Undo {action}" as a whole phrase; a catalog
    /// without that entry (Spanish) keeps the verb in front of the action.
    #[test]
    fn undo_and_redo_labels_use_the_catalog_phrase_when_there_is_one() {
        let es = Lang::from_code("es").expect("es registered");
        set_current(es);
        assert_eq!(command_label("Undo Untranslated custom action"), format!("{} Untranslated custom action", tr(es, "Undo")));
        set_current(Lang::EN);
        assert_eq!(command_label("Undo Untranslated custom action"), "Undo Untranslated custom action");
    }

    /// A label that reads differently by use has its own contextual entry where a language needs
    /// one; a language without it falls back to the plain translation.
    #[test]
    fn contextual_labels_fall_back_to_the_plain_translation() {
        // A context no catalog defines, so the fallback stays tested as catalogs fill their gaps.
        let es = Lang::from_code("es").expect("es registered");
        assert_eq!(tr_ctx(es, "no such context", "Type"), tr(es, "Type"));
    }

    /// Turkish covers every registered menu title and menu command label, keeps ellipses, and
    /// persists in preferences.
    #[test]
    fn turkish_covers_every_menu_label() {
        let tr_lang = Lang::from_code("tr").expect("tr registered");
        assert_eq!(tr_lang.name(), "Türkçe");
        assert_eq!(tr(tr_lang, "File"), "Dosya");
        assert_eq!(tr(tr_lang, "Türkçe Belge.pdf"), "Türkçe Belge.pdf");
        assert_eq!((0..=6).map(|n| (tr_lang.0.plural)(n)).collect::<Vec<_>>(), [0; 7]);
        for spec in pdfcraft_engine::commands::COMMANDS {
            let Some(menu) = spec.menu else { continue };
            assert!(has(tr_lang, menu), "Turkish lacks the menu title {menu:?}");
            assert!(has(tr_lang, spec.label), "Turkish lacks the {menu} menu label {:?} ({})", spec.label, spec.id);
        }
        const KEEP_AS_IS: &[&str] = &[
            "OK",
            "PDF/A…",
            "Microsoft Word (.docx)",
            "PostScript / EPS",
            "Preflight",
            "JavaScript",
            "ZIP",
            "{n} {kind}",
            "{rule} - {status}",
            "Poster",
            "{field}: {e}",
            "`{command}` {when}",
            "{0}: {1}",
            "{month} {y}",
            "+LOC",
            "−LOC",
            "ΔLOC",
            "+Bin",
            "−Bin",
            "Model",
            "Home / End",
            // Apple's name for the translucent look, kept as the name in every language.
            "Liquid Glass",
            // The distinguished name of a certificate, abbreviated as in X.500.
            "DN: ",
        ];
        let (entries, errors) = parse_entries(tr_lang.0.source, tr_lang.0.plural_forms());
        assert!(errors.is_empty(), "Turkish catalog errors: {errors:?}");
        for e in &entries {
            assert!(
                e.source != e.translation
                    || KEEP_AS_IS.contains(&e.source.as_str())
                    || (e.context == "key" && KEY_CAP_NAMES.contains(&e.source.as_str())),
                "{:?} is untranslated",
                e.source
            );
            assert!(!e.translation.contains("..."), "use … rather than three dots: {:?}", e.translation);
            assert_eq!(e.source.starts_with(' '), e.translation.starts_with(' '), "leading space mismatch: {:?}", e.translation);
            assert_eq!(e.source.ends_with(' '), e.translation.ends_with(' '), "trailing space mismatch: {:?}", e.translation);
        }
        let mut app = crate::PdfCraftApp::default();
        app.set_option("language", "tr").unwrap();
        assert!(app.set_option("language", "tu").is_err());
        assert_eq!(app.language, "tr");
        let mut restored = crate::PdfCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, "tr");
    }

    /// Turkish translates every registered command, menu and All tools label.
    #[test]
    fn turkish_covers_commands_and_catalogue() {
        let tr_lang = Lang::from_code("tr").expect("tr registered");
        for command in pdfcraft_engine::commands::COMMANDS {
            assert!(has(tr_lang, command.label), "missing command: {}", command.label);
            if let Some(menu) = command.menu {
                assert!(has(tr_lang, menu), "missing menu: {menu}");
            }
        }
        for group in pdfcraft_engine::catalog::TOOL_GROUPS {
            assert!(has(tr_lang, group.label), "missing group: {}", group.label);
            for section in group.sections {
                assert!(has(tr_lang, section.title), "missing section: {}", section.title);
                for item in section.items {
                    assert!(has(tr_lang, item.label), "missing item: {}", item.label);
                }
            }
        }
    }

    /// Turkish translates every `tl!("…")` literal in the UI source.
    #[test]
    fn turkish_covers_ui_literals() {
        let literals = ui_literals();
        assert!(literals.len() > 900, "source scan found only {} literals", literals.len());
        let tr_lang = Lang::from_code("tr").expect("tr registered");
        let missing: Vec<_> = literals.iter().filter(|label| !has(tr_lang, label)).collect();
        assert!(missing.is_empty(), "untranslated Turkish UI literals: {missing:#?}");
    }

    #[test]
    fn french_is_registered() {
        let fr = Lang::from_code("fr").expect("fr registered");
        assert_eq!(fr.name(), "Français");
        assert_eq!(normalize_pref("FR"), Some("fr"));
        assert_eq!(lang_from_tag("fr_FR.UTF-8"), Some(fr));
        assert_eq!(lang_from_tag("fr-CA"), Some(fr));
        assert_eq!(first_supported("fr-FR\r\nen-US"), Some(fr));
        assert_eq!(tr(fr, "File"), "Fichier");
        assert_eq!(tr(fr, "Save as…"), "Enregistrer sous…");
        assert_eq!(tr(fr, "Bookmarks"), "Signets");
        assert_eq!(tr(fr, "Layers"), "Calques");
        assert_eq!(tr(fr, "Rapport de l'utilisateur.pdf"), "Rapport de l'utilisateur.pdf");
        assert_eq!((0..=3).map(|n| (fr.0.plural)(n)).collect::<Vec<_>>(), [0, 0, 1, 1]);
        assert_eq!(trn(fr, 0, "{n} page", "{n} pages"), "0 page");
        assert_eq!(trn(fr, 1, "{n} page", "{n} pages"), "1 page");
        assert_eq!(trn(fr, 2, "{n} page", "{n} pages"), "2 pages");
        assert_eq!(trn(fr, 1, "{n} field", "{n} fields"), "1 champ");
        assert_eq!(trn(fr, 2, "{n} field", "{n} fields"), "2 champs");
        let mut app = crate::PdfCraftApp::default();
        app.set_option("language", "fr").unwrap();
        assert_eq!(app.language, "fr");
        let mut restored = crate::PdfCraftApp::default();
        restored.restore(&app.persist());
        assert_eq!(restored.language, "fr");
    }

    /// French translates every registered command and every All tools group, section and item.
    #[test]
    fn french_covers_commands_and_catalogue() {
        let fr = Lang::from_code("fr").expect("fr registered");
        for command in pdfcraft_engine::commands::COMMANDS {
            assert!(has(fr, command.label), "missing command: {}", command.label);
            if let Some(menu) = command.menu {
                assert!(has(fr, menu), "missing menu: {menu}");
            }
        }
        for group in pdfcraft_engine::catalog::TOOL_GROUPS {
            assert!(has(fr, group.label), "missing group: {}", group.label);
            for section in group.sections {
                assert!(has(fr, section.title), "missing section: {}", section.title);
                for item in section.items {
                    assert!(has(fr, item.label), "missing item: {}", item.label);
                }
            }
        }
    }

    /// New tl!("literal") labels must not silently fall back to English.
    #[test]
    fn french_covers_ui_literals() {
        let fr = Lang::from_code("fr").expect("fr registered");
        let mut stack = vec![std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src")];
        let mut literals = std::collections::BTreeSet::new();
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(dir).expect("UI source directory") {
                let path = entry.expect("UI source entry").path();
                if path.is_dir() {
                    if path.file_name().is_some_and(|name| name != "i18n") {
                        stack.push(path);
                    }
                } else if path.extension().is_some_and(|ext| ext == "rs") {
                    let source = std::fs::read_to_string(path).expect("UI source file").replace("\r\n", "\n");
                    let mut rest = source.split("#[cfg(test)]\nmod ").next().unwrap_or_default();
                    while let Some((_, after)) = rest.split_once("tl!(\"") {
                        let mut escaped = false;
                        let end = after
                            .char_indices()
                            .find_map(|(i, c)| {
                                if c == '"' && !escaped {
                                    return Some(i);
                                }
                                escaped = c == '\\' && !escaped;
                                None
                            })
                            .expect("closed tl! literal");
                        let (raw, tail) = after.split_at(end);
                        if tail.starts_with("\")") {
                            let label: String = serde_json::from_str(&format!("\"{raw}\"")).expect("UI literal escapes");
                            literals.insert(label);
                        }
                        rest = tail.strip_prefix('"').expect("closing quote");
                    }
                }
            }
        }
        assert!(literals.len() > 900, "source scan found only {} literals", literals.len());
        let missing: Vec<_> = literals.iter().filter(|label| !has(fr, label)).collect();
        assert!(missing.is_empty(), "untranslated French UI literals: {missing:#?}");
    }

    #[test]
    fn french_history_and_diagnostics_preserve_user_values() {
        let fr = Lang::from_code("fr").expect("fr registered");
        set_current(fr);
        assert_eq!(command_label("Undo Insert pages from Rapport {n}.pdf"), "Annuler Insérer des pages depuis Rapport {n}.pdf");
        assert_eq!(command_label("Redo Fill in Contact {key}"), "Rétablir Remplir Contact {key}");
        assert_eq!(action_label("Change Title"), "Modifier Titre");
        assert_eq!(action_label("Custom action {n}"), "Custom action {n}");
        assert_eq!(fmt(t("This page couldn't be displayed.\n{e}"), &[("e", "OS error {n}")]), "Impossible d'afficher cette page.\nOS error {n}");
        assert_eq!(fmt(t("{n} pages selected"), &[("n", "3")]), "3 pages sélectionnées");
        assert_eq!(tr(fr, "CheckBox"), "Case à cocher");
        set_current(Lang::EN);
    }

    #[test]
    fn tool_names_inside_sentences_are_lowercased() {
        set_current(Lang::EN);
        assert_eq!(in_sentence("Text Box"), "text box");
        set_current(Lang::from_code("es").expect("es registered"));
        assert_eq!(in_sentence("Cuadro de texto"), "cuadro de texto");
        set_current(Lang::EN);
    }
    #[test]
    fn turkish_keeps_its_dotted_and_dotless_i_when_changing_case() {
        set_current(Lang::from_code("tr").expect("tr registered"));
        assert_eq!(upper_label("Advanced"), "GELİŞMİŞ");
        assert_eq!(upper_label("Signatures"), "İMZALAR");
        // A label with no Turkish row is still English and keeps the English rule.
        assert_eq!(upper_label("Untranslated title"), "UNTRANSLATED TITLE");
        assert_eq!(in_sentence("İmza Alanı"), "imza alanı");
        assert_eq!(in_sentence("Işık"), "ışık");
        set_current(Lang::EN);
        assert_eq!(upper_label("Advanced"), "ADVANCED");
        assert_eq!(in_sentence("Image"), "image");
    }

    /// Every bundled catalog is well-formed and consistent with its sources.
    #[test]
    fn bundled_catalogs_are_consistent() {
        let mut codes = std::collections::HashSet::new();
        for l in &LANGUAGES {
            assert!(codes.insert(l.code), "duplicate code {}", l.code);
            assert!(l.code == l.code.to_ascii_lowercase() && !l.name.is_empty(), "{}", l.code);
            let (entries, errors) = parse_entries(l.source, l.plural_forms());
            assert!(errors.is_empty(), "{}: {errors:?}", l.code);
            for e in entries.iter().filter(|e| e.context == "@id") {
                let command = pdfcraft_engine::commands::command(&e.source);
                let Some(command) = command else { panic!("{}: unknown command id {:?}", l.code, e.source) };
                assert_eq!(placeholders(&e.translation), placeholders(command.label), "{}: {}", l.code, e.source);
                assert_eq!(e.translation.ends_with('…'), command.label.ends_with('…'), "{}: ellipsis mismatch: {}", l.code, e.source);
            }
        }
    }

    /// Preferences ▸ Date format ▸ Language offers exactly the interface languages, in order.
    #[test]
    fn date_languages_are_the_interface_languages() {
        let interface: Vec<(&str, &str)> = Lang::all().map(|l| (l.code(), l.name())).collect();
        let dates: Vec<(&str, &str)> = pdfcraft_engine::dates::DATE_LANGUAGES.iter().map(|l| (l.code, l.name)).collect();
        assert_eq!(dates, interface);
    }
}
