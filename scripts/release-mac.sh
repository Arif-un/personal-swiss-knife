#!/usr/bin/env bash
set -euo pipefail

# Build the macOS app bundle and install it into /Applications.
APP_NAME="personal-swiss-knife"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BUNDLE_APP="$ROOT/src-tauri/target/release/bundle/macos/$APP_NAME.app"
DEST_DIR="$HOME/Applications"
DEST="$DEST_DIR/$APP_NAME.app"

echo "==> Building $APP_NAME (release)"
pnpm tauri build

if [[ ! -d "$BUNDLE_APP" ]]; then
  echo "error: bundle not found at $BUNDLE_APP" >&2
  exit 1
fi

echo "==> Removing old $DEST"
mkdir -p "$DEST_DIR"
rm -rf "$DEST"

echo "==> Installing to $DEST"
cp -R "$BUNDLE_APP" "$DEST"

echo "==> Clearing quarantine attributes"
xattr -cr "$DEST"

echo "==> Done: $DEST"
