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
  for plugin in "$here"/plugins/*; do
    [[ -d "$plugin" ]] && widgets+=("${plugin##*/}")
  done
fi

# Validate before touching installed widgets; build the cursor's native helper
# before making its bar item live.
for w in "${widgets[@]}"; do
  [[ "$w" =~ ^[a-z0-9]+([.-][a-z0-9]+)+$ && -d "$here/plugins/$w" ]] || { echo "unknown widget: $w" >&2; exit 1; }
done
for w in "${widgets[@]}"; do
  if [[ "$w" == dev.cursor ]]; then
    command -v cargo >/dev/null || { echo 'dev.cursor requires Rust 1.95+ (cargo).' >&2; exit 1; }
    (cd "$here/apps/omarchy-cursor" && cargo run --release --locked -- install --widget)
  fi
done

mkdir -p "$dest"
for w in "${widgets[@]}"; do
  rm -rf "$dest/$w"
  cp -r "$here/plugins/$w" "$dest/$w"
  echo "installed $w -> $dest/$w"
done

# Append to the bar layout unless already present. Everything else in
# shell.json is left untouched.
if [[ -f "$shell_json" ]] && command -v jq >/dev/null; then
  backup_done=false
  for w in "${widgets[@]}"; do
    if ! jq -e --arg id "$w" '[.bar.layout[][]?.id] | index($id)' "$shell_json" >/dev/null; then
      if [[ "$backup_done" == false ]]; then
        cp -p "$shell_json" "$shell_json.bak.$(date +%s)"
        backup_done=true
      fi
      tmp="$(mktemp "${shell_json}.tmp.XXXXXX")"
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
