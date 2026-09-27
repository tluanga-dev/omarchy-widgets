# Omarchy Cursor

A standalone Rust cursor highlighter and magnifier for Omarchy 4 / Hyprland 0.56.
Inspired by [Cursor Pro](https://appahead.studio/apps/cursor-pro/) and its
[App Store feature list](https://apps.apple.com/in/app/cursor-pro/id1447043133).
This is an independent Linux implementation, with its own name, icon, and code.

## Use

From this repository, install the bar widget with `./install.sh dev.cursor`.
Click the labeled **Cursor** button in the top bar to open its controls;
**Preferences** opens the full editor. New installations use the center section.
For an existing installation, move it beside Screenshot with
`omarchy bar move dev.cursor --section center --after dev.screenshot`.
This mode hides the standalone tray icon. The widget starts the user service
when loaded unless its `autoStart` setting is false.

Open **Omarchy Cursor** from the application launcher, or run:

```sh
~/.local/bin/omarchy-cursor
```

| Shortcut | Action |
|---|---|
| Super + Alt + C | Toggle highlighting |
| Super + Alt + M | Toggle the magnifier |
| Super + Alt + L | Locate the pointer with a pulse |

Closing preferences leaves the overlay running. In widget mode, use **Stop cursor
helper** in the popup to stop it. A standalone installation provides a tray icon:
right-click for its menu, middle-click to toggle highlighting, or use **Quit
Omarchy Cursor** to stop it. Behavior preferences include launch at login,
custom shortcuts, and a hold-to-magnify option. Click **Apply shortcuts** after
changing shortcuts or the hold option. Standalone login startup is off initially;
the widget's separate `autoStart` setting is on by default.

## Features

- Circle, squircle, rhombus, and rectangle highlights.
- Adjustable size, border weight, solid/dashed/dotted borders, and glow.
- Separate highlight, left-click, and right-click RGBA colors.
- Press/release animation and subtle motion warping.
- Automatic idle hiding, attention pulses, and an explicit locate shortcut.
- Live magnification from 1.5× to 10×, adjustable lens size, smooth or crisp sampling.
- Appearance, Magnifier, and Behavior preferences with an interactive preview.
- A system tray, launcher entry, local CLI, and optional login startup.
- One click-through Wayland layer per output, with physical-pixel rendering at
  fractional scales. Monitor geometry refreshes on output events and every two seconds.

The lens sits **beside** the pointer. This is a deliberate Linux adaptation:
capturing underneath a centered lens would capture the lens itself. The highlight
temporarily disappears during magnification so it does not contaminate the source.
At display edges, unavailable pixels have a dark fill and the crosshair remains
aligned to the pointer. The lens and halo appear in full-display screen captures;
sharing one application window alone normally excludes desktop overlays.

The macOS menu bar and Apple Shortcuts are replaced by a Linux tray, Hyprland
bindings, and CLI commands. This does not reproduce macOS system chrome or claim
pixel-identical behavior to the proprietary application.

## Build and install

Requires Rust 1.95+, a working OpenGL/Wayland desktop, Omarchy's Lua Hyprland
configuration, and a systemd user session. Development was tested with Rust 1.98,
Omarchy 4.0.4, and Hyprland 0.56.2. Cargo.lock pins the Rust dependencies. No extra
system packages were required on the development machine.

```sh
cargo build --release --locked
./target/release/omarchy-cursor install
~/.local/bin/omarchy-cursor
```

Installation writes only user files:

- `~/.local/bin/omarchy-cursor`
- `~/.local/share/applications/omarchy-cursor.desktop`
- `~/.local/share/icons/hicolor/scalable/apps/omarchy-cursor.svg`
- `~/.config/omarchy-cursor/config.json`
- `~/.config/hypr/omarchy-cursor.lua`
- `~/.config/systemd/user/omarchy-cursor.service`

It backs up `hyprland.lua`, adds a marked require block, checks shortcut conflicts,
and reloads/validates Hyprland. Mouse bindings are non-consuming, accept modifiers,
and register both press and release. The program never opens `/dev/input`, requires
root, replaces the mouse theme, or reads keyboard events. Installation does not
enable login startup. Config/state use XDG_CONFIG_HOME and XDG_RUNTIME_DIR.

For a widget installation, disable the widget before removing its helper:

```sh
omarchy plugin disable dev.cursor
```

Remove the native application and its integration, preserving preferences:

```sh
~/.local/bin/omarchy-cursor uninstall
```

The disabled widget files remain in `~/.config/omarchy/plugins/dev.cursor`.
Reinstall with `./install.sh dev.cursor` and re-enable with
`omarchy plugin enable dev.cursor` if previously disabled.

## CLI and diagnostics

```sh
omarchy-cursor settings
omarchy-cursor start
omarchy-cursor toggle
omarchy-cursor magnify
omarchy-cursor locate
omarchy-cursor status
omarchy-cursor reload
omarchy-cursor quit
omarchy-cursor doctor
journalctl --user -u omarchy-cursor.service
```

Launching preferences starts the user service when installed. `daemon` runs it
directly for development. Control uses a Unix socket in a private runtime directory
and a process lock prevents duplicate overlays. Preferences are validated and
written atomically; malformed preferences produce an error instead of being reset.

The magnifier uses Hyprland's supported `zwlr_screencopy_manager_v1` protocol, only
while active, with at most one region capture in flight. Screen data stays in RAM.
No network requests, analytics, screenshots, or input histories are generated by
the application. `status` reports capture availability, frame count, and errors.

## Verify

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --release --locked
```

`examples/input_probe.rs` and `examples/keyboard_probe.rs` are explicit manual
integration tools that **inject** mouse clicks and Super+Alt+C/M/L for testing.
They are not part of the installed app. The keyboard probe uses `xkbcli` for a
standard evdev map; wtype's generated keycodes do not match Hyprland's default
physical-key bindings. Run these only with a suitable window focused.

With preferences closed and the desktop unlocked, `cargo build --release --examples`
followed by `python3 scripts/live-smoke.py` checks the installed app on each display.
It moves the pointer, injects shortcuts, and temporarily changes preferences and
bindings; a `finally` block restores them. Run it while you are not using the mouse.

`python3 scripts/service-smoke.py` tests the installed helper and widget adapter
without injecting input. It briefly toggles highlighting and restarts the service,
then restores preferences.

See [VERIFICATION.md](VERIFICATION.md) for observed results and limits.

## Implementation

- `overlay.rs`: Smithay layer-shell surfaces, display mapping, IPC event loop.
- `capture.rs`: bounded shared-memory region capture and pixel conversion.
- `render.rs`: tiny-skia highlight and magnifier drawing, shared with the preview.
- `settings.rs`: native egui/OpenGL preferences; no webview.
- `tray.rs`: StatusNotifierItem tray through ksni.
- `hypr.rs`, `install.rs`, `config.rs`, `ipc.rs`: compositor integration and lifecycle.

Public API references: [Hyprland IPC](https://wiki.hypr.land/IPC/),
[Lua bindings](https://wiki.hypr.land/Configuring/Basics/Binds/),
[layer rules](https://wiki.hypr.land/Configuring/Basics/Window-Rules/),
[Smithay toolkit](https://docs.rs/smithay-client-toolkit/0.20.0/smithay_client_toolkit/).
