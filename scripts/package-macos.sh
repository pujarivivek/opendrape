#!/usr/bin/env bash
# Builds a universal (Apple Silicon + Intel) OpenDrape.app and a drag-to-install DMG.
# Usage: scripts/package-macos.sh [name-suffix]   → dist/OpenDrape-<suffix>-macos-universal.dmg
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION=$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)
SUFFIX="${1:-$VERSION}"
UNIVERSAL=target/universal-apple-darwin/release

rustup target add aarch64-apple-darwin x86_64-apple-darwin
cargo build --release -p opendrape --target aarch64-apple-darwin
cargo build --release -p opendrape --target x86_64-apple-darwin
mkdir -p "$UNIVERSAL"
lipo -create -output "$UNIVERSAL/opendrape" \
  target/aarch64-apple-darwin/release/opendrape \
  target/x86_64-apple-darwin/release/opendrape

cargo packager --release -p opendrape --target universal-apple-darwin --formats app
APP="$UNIVERSAL/OpenDrape.app"
# Ad-hoc signature: without any signature, Apple Silicon reports a downloaded app as
# "damaged" instead of offering "Open Anyway".
codesign --force --deep --sign - "$APP"

# Plain hdiutil instead of the packager's DMG step, which scripts Finder and can time out.
STAGE=$(mktemp -d)
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
mkdir -p dist
DMG="dist/OpenDrape-$SUFFIX-macos-universal.dmg"
hdiutil create -quiet -volname OpenDrape -srcfolder "$STAGE" -ov -format UDZO "$DMG"
rm -rf "$STAGE"
echo "$DMG"
