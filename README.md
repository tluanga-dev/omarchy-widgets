# omarchy-widgets

Custom status-bar widgets for the [Omarchy](https://omarchy.org/) shell
(Quickshell). Each widget is a self-contained plugin directory under
`plugins/` that drops into `~/.config/omarchy/plugins/`.

| Widget | What it does | Right-click |
|---|---|---|
| `dev.screenshot` | Camera icon with a popup menu: **Select area**, **Window**, **Full screen** (each copies to the clipboard *and* saves a PNG), **Copy text** (OCR), **Open folder** | Start an area capture immediately |
| `dev.cpu` | Live CPU % pill; popup shows load, clock, temperature and the top CPU processes | Open `btop` |
| `dev.ram` | Live memory usage; popup shows totals and the top memory processes | Open `btop` |
| `dev.disk` | Live disk usage for a mount; popup lists filesystems and top I/O processes | Open `btop` |

All popups support keyboard navigation (arrows / Enter / Esc / Tab between panels).

## Install

```bash
git clone git@github.com:tluanga-dev/omarchy-widgets.git
cd omarchy-widgets
./install.sh              # all widgets
./install.sh dev.screenshot   # or just one
```

`install.sh` copies the plugin directories into `~/.config/omarchy/plugins/`
and appends any widget not already in your bar to the right section of
`~/.config/omarchy/shell.json`. Move them around with:

```bash
omarchy bar move dev.screenshot --section right
```

## Settings

Each widget's `manifest.json` declares its settings; they show up in the
bar's widget settings UI and can also be set inline in `shell.json`:

```jsonc
{ "id": "dev.screenshot", "directory": "~/Documents/Screenshots" }
{ "id": "dev.cpu",  "interval": 1, "groupByName": true, "topCount": 5 }
{ "id": "dev.ram",  "interval": 1, "showPercent": true, "groupByName": true, "topCount": 5 }
{ "id": "dev.disk", "interval": 1, "mount": "/", "groupByName": true, "topCount": 5 }
```

## Requirements

- Omarchy with the Quickshell-based `omarchy-shell`
- `dev.screenshot` uses Omarchy's own capture tools (`omarchy-capture-screenshot`,
  `omarchy-capture-text`, `grim`, `slurp`, `wl-copy`) — all ship with Omarchy
- `jq` (for `install.sh` to edit `shell.json`; ships with Omarchy)

## Development notes

- Plugin code under `~/.config/omarchy/plugins/` hot-reloads on save, but an
  edit to an already-loaded QML file can keep being served from Qt's
  component cache. If a change seems ignored, `omarchy restart shell`.
- Shell log for debugging QML errors:
  `/run/user/$UID/quickshell/by-pid/$(pgrep -f 'quickshell -n -p')/log.log`
- Use `Util.shellQuote()` (from `qs.Commons`) when building shell commands
  for `bar.run()`; the bar API itself has no quoting helper.

## License

MIT
