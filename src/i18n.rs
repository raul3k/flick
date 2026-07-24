//! Language selection: which locales exist, and how the active one is picked.

use crate::config::AUTO;

/// Supported locales, as (code, name shown to the user). Names stay in their
/// own language on purpose: someone looking for "Español" should not have to
/// read the current language to find it.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", "English"),
    ("es", "Español"),
    ("pt-BR", "Português (Brasil)"),
];

/// Applies a language setting, which is either [`AUTO`] or a code from
/// [`LANGUAGES`].
pub fn apply(setting: &str) {
    let locale = if setting == AUTO {
        from_system()
    } else {
        setting.to_string()
    };
    rust_i18n::set_locale(&locale);
}

/// Maps the system locale onto a supported one: exact match first, then the
/// language alone (so `pt-PT` still gets Portuguese), then English.
fn from_system() -> String {
    let system = sys_locale::get_locale()
        .unwrap_or_default()
        .replace('_', "-");
    let language = system.split('-').next().unwrap_or_default();

    let exact = LANGUAGES
        .iter()
        .find(|(code, _)| code.eq_ignore_ascii_case(&system));
    let by_language = || {
        LANGUAGES.iter().find(|(code, _)| {
            code.split('-')
                .next()
                .unwrap_or_default()
                .eq_ignore_ascii_case(language)
        })
    };

    exact
        .or_else(by_language)
        .map(|(code, _)| (*code).to_string())
        .unwrap_or_else(|| "en".to_string())
}

/// Position of a setting in the menu, where 0 is [`AUTO`] and the rest follow
/// [`LANGUAGES`].
pub fn menu_index(setting: &str) -> usize {
    LANGUAGES
        .iter()
        .position(|(code, _)| code == &setting)
        .map(|i| i + 1)
        .unwrap_or(0)
}

/// Inverse of [`menu_index`].
pub fn setting_at(index: usize) -> String {
    match index.checked_sub(1) {
        Some(i) => LANGUAGES
            .get(i)
            .map(|(code, _)| (*code).to_string())
            .unwrap_or_else(|| AUTO.to_string()),
        None => AUTO.to_string(),
    }
}
