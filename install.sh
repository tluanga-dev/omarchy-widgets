#!/usr/bin/env bash
# Install (or update) these widgets into the Omarchy user plugin directory
# and add any that are missing to the bar's right section.
#
#   ./install.sh            # install all widgets
#   ./install.sh dev.cpu    # install only the named widget(s)
set -euo pipefail

here="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
dest="${OMARCHY_PLUGIN_DIR:-$HOME/.config/omarchy/plugins}"
shell_json="$HOME/.config/omarchy/shell.json"

widgets=("$@")
if [[ ${#widgets[@]} -eq 0 ]]; then
  mapfile -t widgets < <(ls "$here/plugins")
fi

mkdir -p "$dest"
for w in "${widgets[@]}"; do
  [[ -d "$here/plugins/$w" ]] || { echo "unknown widget: $w" >&2; exit 1; }
  rm -rf "$dest/$w"
  cp -r "$here/plugins/$w" "$dest/$w"
  echo "installed $w -> $dest/$w"
done

# Append to the bar layout unless already present. Everything else in
# shell.json is left untouched.
if [[ -f "$shell_json" ]] && command -v jq >/dev/null; then
  for w in "${widgets[@]}"; do
    if ! jq -e --arg id "$w" '[.bar.layout[][]?.id] | index($id)' "$shell_json" >/dev/null; then
      tmp="$(mktemp)"
      jq --arg id "$w" '.bar.layout.right = ((.bar.layout.right // []) + [{id: $id}])' "$shell_json" > "$tmp"
      mv "$tmp" "$shell_json"
      echo "added $w to the bar (right section)"
    fi
  done
else
  echo "note: add the widgets to ~/.config/omarchy/shell.json manually (jq or shell.json not found)"
fi

echo
echo "Plugin code hot-reloads, but a fresh edit to an already-loaded widget"
echo "may be served from the QML cache. If a change doesn't show, run:"
echo "  omarchy restart shell"
