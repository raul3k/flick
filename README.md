# Flick

[![CI](https://github.com/raul3k/flick/actions/workflows/ci.yml/badge.svg)](https://github.com/raul3k/flick/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Mute and unmute your microphone on Linux, from a scriptable CLI, a tray indicator, or a small GTK window whose status stays in sync with the real state in real time.

<!-- TODO: add a screenshot at docs/screenshot.png and reference it here -->

## Features

- Mute, unmute, or toggle your default microphone.
- A compact GTK4 window: a green/red indicator, a status label, and a switch.
- A **tray indicator** next to the clock: a green dot when the mic is live, red when it is muted. It stays out of the way and tells you the state at a glance.
- The window never lies: it reads the real mic state and **updates live** whenever the mic changes from anywhere (GNOME, another app, a keyboard shortcut), using PipeWire events rather than polling.
- One binary, three modes: a scriptable CLI, the tray indicator, and the GUI.
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

Launch **Flick** from your applications menu, or run `flick` with no arguments.

### Tray indicator

```bash
flick tray
```

Runs in the background and puts a dot next to the clock: **green** when the mic is live, **red** when it is muted, following the real state in real time. Click it to open the window; right-click for a menu with **Abrir Flick** and **Sair**.

The `.deb` installs an autostart entry, so after installing it the indicator comes back on every login with no extra setup.

### Command line

```bash
flick status   # print the current state
flick on       # unmute
flick off      # mute
flick toggle   # flip the current state
```

### Bind it to a key (optional)

Flick does not register a global shortcut itself, but since `flick toggle` is just a command you can bind it in GNOME: **Settings -> Keyboard -> Custom Shortcuts**, with the command `flick toggle`. If the window is open, it reflects the change instantly.

## How it works

- The mic state is read from `wpctl` (the mute of `@DEFAULT_AUDIO_SOURCE@`).
- A background thread connects to PipeWire and listens for changes on audio source nodes. When the mic changes from anywhere, the window re-reads the real state and refreshes. UI updates are marshalled back to the main thread through an async channel, so the PipeWire thread never touches widgets.
- The tray indicator speaks `StatusNotifierItem` over D-Bus (no GTK status icon), and its dot is drawn with cairo and handed over as an ARGB32 pixmap, so it scales to whatever size the panel asks for. It listens to the same PipeWire events as the window.

## Building a `.deb`

```bash
cargo install cargo-deb
cargo deb   # produces target/debian/flick_<version>_amd64.deb
```

## Contributing

Contributions are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT (c) 2026 Raul Souza, see [LICENSE](LICENSE).
