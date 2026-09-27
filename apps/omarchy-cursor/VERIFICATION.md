# Verification

Test environment: Omarchy 4.0.4, Hyprland 0.56.2, Rust 1.98.0, Wayland,
systemd user session. Two 2560×1440 outputs at 1.25× scale, positioned side
by side (2048×1152 logical pixels each). Date: 2026-09-27.

## Build checks

- `cargo fmt --check`: passed.
- `cargo clippy --all-targets --locked -- -D warnings`: passed.
- `cargo test --release --locked`: seven tests passed. Covers config defaults,
  invalid input, fractional/rotated/negative monitor geometry, lens placement,
  transparent highlight center, non-consuming press/release bindings, and
  retaining unapplied shortcut drafts when appearance changes.
- `bash -n` on the installer and widget adapter: passed.
- `qmlformat` parsed `plugins/dev.cursor/Panel.qml`; manifest JSON parsed.
- Python integration script compiled; `git diff --check` passed.

## Desktop observations

- Installed the release app into the user account. Hyprland reloaded without
  configuration errors. The native preferences window and preview rendered.
- A highlight followed the pointer and the live magnifier displayed enlarged
  desktop content on both outputs. The lens was offset from its source.
- A Wayland virtual-pointer click passed through the overlay and toggled the
  native preferences checkbox. IPC reported the pressed state, then release;
  the click highlight was visible.
- Super+Alt+C and Super+Alt+M, injected with a standard evdev virtual-keyboard
  map, toggled the actual installed compositor bindings. These observations
  used synthetic input, not a human-operated physical mouse or keyboard.
- Omarchy registered and enabled `dev.cursor`; its adapter read the Rust
  helper's live JSON state. The shell loaded the plugin without cursor-specific
  QML errors. Multi-output IPC-handler duplication warnings also occur for the
  existing shell widgets.
- `scripts/service-smoke.py` passed against the final installed release:
  widget status and highlight toggle, invalid-preferences rejection while
  running, duplicate-daemon exclusion, and three stop/immediate-start cycles
  through the widget adapter and systemd user service. Saved preferences were
  restored and Hyprland configuration remained clean.
- Repeated widget installation retained a single `dev.cursor` bar entry and
  restarted the helper with the updated binary.
- After unlocking, verified the labeled **Cursor** button in the top-center
  bar beside Screenshot. A virtual-pointer left click opened the themed popup;
  right-click toggled highlighting. Arrow/Enter navigation toggled highlighting
  and the magnifier, whose capture frame count increased. Escape dismissed it.
  Clicking **Preferences** opened the native editor. The captured popup is
  [`docs/cursor-widget.png`](../../docs/cursor-widget.png).
- Cleared the shell's QML component cache with `omarchy restart shell` after
  updating the button. The new label and popup rendered without QML errors.
  Transient compositor errors now clear after the next successful refresh.

## Limits

All four display corners, idle hiding, attention pulses, middle-click on the
bar, and hold-mode release have not completed an end-to-end desktop test.
`scripts/live-smoke.py` contains the repeatable display/shortcut/
idle/hold checks and refuses to run on a locked desktop or with preferences open.

Hot-plugging, physical display rotation, HDR, full logout/reboot startup,
non-US keyboard layouts, and older Hyprland versions were not tested.
Geometry unit tests do not establish rotated-output capture correctness.
The Linux lens is intentionally beside the cursor; this is an independent
implementation, not a pixel-identical macOS port.
