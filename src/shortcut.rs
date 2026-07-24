//! Global toggle shortcut.
//!
//! On Wayland an application cannot grab a global shortcut on its own, so on
//! GNOME we register a custom keybinding in the shell's settings that runs
//! `flick toggle`. The shell owns the grab; we only register it. Other
//! desktops (and Windows, via `RegisterHotKey`) would plug in here behind the
//! same [`apply`] entry point.

use gtk::gio;
use gtk::prelude::*;

const MEDIA_KEYS: &str = "org.gnome.settings-daemon.plugins.media-keys";
const CUSTOM_KEYBINDING: &str = "org.gnome.settings-daemon.plugins.media-keys.custom-keybinding";
// A Flick-specific slot, so we never clash with the user's own `customN`
// entries and updates stay idempotent.
const PATH: &str = "/org/gnome/settings-daemon/plugins/media-keys/custom-keybindings/flick/";
const COMMAND: &str = "flick toggle";

/// Whether the desktop exposes the GNOME media-keys schema, i.e. whether we can
/// register a shortcut at all. False on non-GNOME desktops.
pub fn available() -> bool {
    gio::SettingsSchemaSource::default()
        .and_then(|source| source.lookup(MEDIA_KEYS, true))
        .is_some()
}

/// Registers `accel` (a GTK accelerator like `<Super><Alt>m`) as the toggle
/// shortcut, replacing any previous one. An empty `accel` clears it. A no-op
/// where [`available`] is false.
pub fn apply(accel: &str) {
    if !available() {
        return;
    }

    let media = gio::Settings::new(MEDIA_KEYS);
    let mut paths: Vec<String> = media
        .strv("custom-keybindings")
        .iter()
        .map(|s| s.to_string())
        .collect();
    let listed = paths.iter().any(|p| p == PATH);

    if accel.is_empty() {
        if listed {
            paths.retain(|p| p != PATH);
            set_paths(&media, &paths);
        }
        return;
    }

    if !listed {
        paths.push(PATH.to_string());
        set_paths(&media, &paths);
    }
    let keybinding = gio::Settings::with_path(CUSTOM_KEYBINDING, PATH);
    keybinding.set_string("name", "Flick").ok();
    keybinding.set_string("command", COMMAND).ok();
    keybinding.set_string("binding", accel).ok();
}

fn set_paths(media: &gio::Settings, paths: &[String]) {
    let refs: Vec<&str> = paths.iter().map(String::as_str).collect();
    media.set_strv("custom-keybindings", refs).ok();
}

// Desktop-level keybinding schemas worth checking for clashes. Not exhaustive
// (application shortcuts live elsewhere), but it catches the common ones.
const KEYBINDING_SCHEMAS: &[&str] = &[
    "org.gnome.desktop.wm.keybindings",
    "org.gnome.shell.keybindings",
    "org.gnome.mutter.keybindings",
    "org.gnome.mutter.wayland.keybindings",
    MEDIA_KEYS,
];

/// If `accel` is already bound by a known GNOME schema (or the user's own
/// custom keybindings), returns the name of what holds it. Best-effort.
pub fn conflict(accel: &str) -> Option<String> {
    let target = normalize(accel)?;
    let source = gio::SettingsSchemaSource::default()?;

    for id in KEYBINDING_SCHEMAS {
        let Some(schema) = source.lookup(id, true) else {
            continue;
        };
        let settings = gio::Settings::new(id);
        for key in schema.list_keys() {
            if bound_accels(&settings, key.as_str())
                .iter()
                .any(|b| normalize(b).as_deref() == Some(target.as_str()))
            {
                return Some(key.to_string());
            }
        }
    }

    conflict_in_custom(&target)
}

/// Canonical form of an accelerator, so `<Primary>a` and `<Control>A` compare
/// equal. `None` for anything that is not a real accelerator.
fn normalize(accel: &str) -> Option<String> {
    let (key, mods) = gtk::accelerator_parse(accel)?;
    Some(gtk::accelerator_name(key, mods).to_string())
}

/// The accelerator strings held by one settings key, whether it stores a single
/// binding (`s`) or a list of them (`as`).
fn bound_accels(settings: &gio::Settings, key: &str) -> Vec<String> {
    let value = settings.value(key);
    match value.type_().as_str() {
        "s" => value.str().map(|s| vec![s.to_string()]).unwrap_or_default(),
        "as" => (0..value.n_children())
            .filter_map(|i| value.child_value(i).str().map(String::from))
            .collect(),
        _ => Vec::new(),
    }
}

fn conflict_in_custom(target: &str) -> Option<String> {
    if !available() {
        return None;
    }
    let media = gio::Settings::new(MEDIA_KEYS);
    for path in media.strv("custom-keybindings") {
        if path == PATH {
            continue; // that one is ours
        }
        let keybinding = gio::Settings::with_path(CUSTOM_KEYBINDING, &path);
        if normalize(&keybinding.string("binding")).as_deref() == Some(target) {
            let name = keybinding.string("name");
            return Some(if name.is_empty() {
                path.to_string()
            } else {
                name.to_string()
            });
        }
    }
    None
}
