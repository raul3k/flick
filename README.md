# Flick

[![CI](https://github.com/raul3k/flick/actions/workflows/ci.yml/badge.svg)](https://github.com/raul3k/flick/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Mute and unmute your microphone on Linux, from a scriptable CLI, a tray indicator, or a small GTK window whose status stays in sync with the real state in real time.

<!-- TODO: add a screenshot at docs/screenshot.png and reference it here -->

## Features

- Mute, unmute, or toggle your default microphone.
- A compact GTK4 window: a green/red indicator, a status label, and a switch.
- A **tray indicator** next to the clock: an "F" tile, green when the mic is live, red and cut by a diagonal when it is muted. The cut means the state is readable without relying on color, which matters because green and red are the pair most affected by color blindness.
- The window never lies: it reads the real mic state and **updates live** whenever the mic changes from anywhere (GNOME, another app, a keyboard shortcut), using PipeWire events rather than polling.
- One binary, three modes: a scriptable CLI, the tray indicator, and the GUI.
- A **global shortcut** to toggle the mic, bound from within the app (on GNOME).
- Speaks **English, Spanish and Portuguese (Brazil)**, following your system language, with a picker in Preferences if you want a different one.
- **Color-blind aware:** the indicator colors can be switched to a palette tuned for the color vision condition you have (red-green or blue-yellow), each measured to keep the two states apart under that condition.
- Small and native (GTK4, talks to PipeWire/WirePlumber).

## Requirements

- Linux running PipeWire with WirePlumber (provides `wpctl`).
- GTK 4.
- For the tray indicator, a desktop that renders `StatusNotifierItem` icons. KDE, Cinnamon and XFCE do it natively. On GNOME you need the **AppIndicator and KStatusNotifierItem Support** extension enabled (it ships with Ubuntu); on Wayland, log out and back in after enabling it.

## Installation

### From the `.deb` (recommended on Debian/Ubuntu)

Grab the latest package from the [Releases](https://github.com/raul3k/flick/releases) page and install it:

```bash
sudo apt install ./flick_*.deb
```

### From source

```bash
git clone https://github.com/raul3k/flick.git
cd flick
cargo build --release
# binary at target/release/flick
```

## Usage

### Window

Launch **Flick** from your applications menu, or run `flick` with no arguments. This opens the window **and** puts the indicator in the tray.

The window has a classic menu bar: **File -> Preferences...** opens a settings dialog (language, colors, and a toggle shortcut), and **File -> Quit** exits. The tray menu opens the same dialog, so there is a single place for settings and nothing to keep in sync.

Closing the window only hides it, so the indicator stays. Use **Quit** to quit for good. Running `flick` again reaches the instance already running instead of starting a second one.

### Tray indicator

```bash
flick tray
```

Same thing without opening the window: it goes straight to the tray and waits there. This is what the autostart entry shipped in the `.deb` runs, so after installing, the indicator is back on every login with no extra setup.

The indicator follows the real state in real time. Click it to open the window; right-click for a menu to open the window, open Preferences, or quit.

### Command line

```bash
flick status   # print the current state
flick on       # unmute
flick off      # mute
flick toggle   # flip the current state
```

### Language

Flick follows your system language, falling back to English when that language is not translated yet. To pick another one, open **Preferences** (from the window's File menu or the tray) and choose it under **Language**. The choice applies immediately and is remembered.

Translations live in [`locales/app.yml`](locales/app.yml), one entry per string with every language side by side, and are compiled into the binary. Adding a language means adding its code to `LANGUAGES` in `src/i18n.rs` and filling that file in. Pull requests with new languages are welcome.

### Colors

Under **Preferences -> Colors** you can switch the indicator palette. Each option shows its two colors and the color vision condition it targets:

- **Green / Red** - the default.
- **Blue / Orange** - Protanopia / Deuteranopia (the red-green deficiencies).
- **Blue / Yellow** - Tritanopia (the blue-yellow deficiency).

The muted icon is also cut by a diagonal, so the state is readable regardless of palette or color vision; the palette is a comfort choice on top of that.

### Shortcut

Under **Preferences -> Shortcut** you can bind a key combination that toggles the mic from anywhere. Click the button, press the combination (it is shown live), and confirm with Enter. Flick warns if the combination is already taken by another desktop shortcut.

How it works, and why it is GNOME-only for now: on Wayland an application cannot grab a global shortcut on its own, so Flick registers a **custom keybinding in the GNOME settings** that runs `flick toggle`. GNOME owns the grab; Flick only registers it (in a dedicated `flick` slot, so it never touches your other shortcuts). On desktops without the GNOME media-keys schema the Shortcut option is hidden. Windows will use `RegisterHotKey` instead; the registration lives behind `src/shortcut.rs` for that.

Note (Wayland): the **Super** key is intentionally not offered. The compositor swallows it before the window can see it, and Super-based bindings tend to collide with GNOME's own. Use `Ctrl`/`Alt`/`Shift` combinations.

The current shortcut is also shown in the tray menu.

### Settings file

Preferences are stored at `~/.config/flick/config.toml` (`$XDG_CONFIG_HOME` is respected). The file is only written once you change something, and an unreadable one is ignored rather than fatal.

```toml
language = "auto"    # or "en", "es", "pt-BR"
palette = "classic"  # or "redgreen", "tritan"
shortcut = ""        # a GTK accelerator, e.g. "<Control><Alt>m"
```

## How it works

- The mic state is read from `wpctl` (the mute of `@DEFAULT_AUDIO_SOURCE@`).
- A background thread connects to PipeWire and listens for changes on audio source nodes. When the mic changes from anywhere, the window re-reads the real state and refreshes. UI updates are marshalled back to the main thread through an async channel, so the PipeWire thread never touches widgets.
- The tray indicator speaks `StatusNotifierItem` over D-Bus (no GTK status icon). Its tile is drawn with cairo and handed over as an ARGB32 pixmap, so the panel can scale it to whatever size it wants. Window and indicator live in the same process and share one PipeWire listener; the tray runs on its own D-Bus thread and asks the main loop to show the window through a channel, so it never touches widgets directly.

## Building a `.deb`

```bash
cargo install cargo-deb
cargo deb   # produces target/debian/flick_<version>_amd64.deb
```

## Contributing

Contributions are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT (c) 2026 Raul Souza, see [LICENSE](LICENSE).
