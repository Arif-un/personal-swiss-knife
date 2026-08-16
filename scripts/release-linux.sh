#!/usr/bin/env bash
set -euo pipefail

# Build the Linux binary and install it into ~/.local/bin, overwriting in place.
# Mirrors release-mac.sh: no updater, each run replaces the installed copy.
APP_NAME="personal-swiss-knife"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BIN="$ROOT/src-tauri/target/release/$APP_NAME"
DEST_DIR="$HOME/.local/bin"
DEST="$DEST_DIR/$APP_NAME"
ICON="$ROOT/src-tauri/icons/128x128.png"
DESKTOP="$HOME/.local/share/applications/$APP_NAME.desktop"

echo "==> Building $APP_NAME (release)"
pnpm tauri build

if [[ ! -f "$BIN" ]]; then
  echo "error: binary not found at $BIN" >&2
  exit 1
fi

echo "==> Installing to $DEST"
mkdir -p "$DEST_DIR"
rm -f "$DEST"          # new inode: a running instance keeps the old file
cp "$BIN" "$DEST"
chmod +x "$DEST"

# One-time desktop entry so it shows in the app launcher.
if [[ ! -f "$DESKTOP" ]]; then
  echo "==> Creating desktop entry $DESKTOP"
  mkdir -p "$(dirname "$DESKTOP")"
  cat >"$DESKTOP" <<EOF
[Desktop Entry]
Type=Application
Name=$APP_NAME
Exec=$DEST
Icon=$ICON
Terminal=false
Categories=Utility;
EOF
fi

echo "==> Done: $DEST"
