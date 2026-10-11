#!/usr/bin/env bash
# Downloads the CC0 fabric scan the cloth's muslin look is baked from, and checks each file's
# MD5 (as Poly Haven publishes them). Poly Haven "Stretch Poplin" (photography colormass,
# processing Rico Cilliers), 1k JPG: its diffuse colour, its normal map (OpenGL convention)
# and its ambient occlusion. License: CC0 1.0 (https://polyhaven.com/license). Baked by
# `cargo xtask fabric`.
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=https://dl.polyhaven.org/file/ph-assets/Textures/jpg/1k/stretch_poplin
OUT=target/fabric
mkdir -p "$OUT"
md5of() { if command -v md5sum >/dev/null; then md5sum "$1" | cut -d' ' -f1; else md5 -q "$1"; fi; }
fetch() {
  FILE="$OUT/$1"
  [ -f "$FILE" ] || curl -fsSL "$BASE/$1" -o "$FILE"
  if [ "$(md5of "$FILE")" != "$2" ]; then echo "checksum mismatch: $FILE" >&2; rm -f "$FILE"; exit 1; fi
  echo "$FILE"
}
fetch stretch_poplin_diff_1k.jpg 03f707a3a0367d2cd3fa90b71eec4b89
fetch stretch_poplin_nor_gl_1k.jpg 930a92aa642c2c4c196cc8c9d26bf65d
fetch stretch_poplin_ao_1k.jpg bbc22d082d95df8a67d1361563840529
