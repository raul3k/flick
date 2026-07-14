# Flick

[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

Mute and unmute your microphone on Linux, from a scriptable CLI or a small GTK window whose status stays in sync with the real state in real time.

<!-- TODO: add a screenshot at docs/screenshot.png and reference it here -->

## Features

- Mute, unmute, or toggle your default microphone.
- A compact GTK4 window: a green/red indicator, a status label, and a switch.
- The window never lies: it reads the real mic state and **updates live** whenever the mic changes from anywhere (GNOME, another app, a keyboard shortcut), using PipeWire events rather than polling.
- One binary, two modes: a scriptable CLI and the GUI.
- Small and native (GTK4, talks to PipeWire/WirePlumber).

## Requirements

- Linux running PipeWire with WirePlumber (provides `wpctl`).
- GTK 4.

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

## Building a `.deb`

```bash
cargo install cargo-deb
cargo deb   # produces target/debian/flick_<version>_amd64.deb
```

## Contributing

Contributions are welcome, see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

MIT (c) 2026 Raul Souza, see [LICENSE](LICENSE).
