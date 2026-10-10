#!/usr/bin/env bash
# Downloads the CC0 studio HDRI the 3D view's lighting is baked from, and checks its MD5
# (as Poly Haven publishes it). Poly Haven "Studio Small 08" by Sergej Majboroda, 1k HDR.
# License: CC0 1.0 (https://polyhaven.com/license). Baked by `cargo xtask studio`.
set -euo pipefail
cd "$(dirname "$0")/.."
URL=https://dl.polyhaven.org/file/ph-assets/HDRIs/hdr/1k/studio_small_08_1k.hdr
MD5=de3ba64222895aca876b1d1c2e0cf81a
OUT=target/studio
FILE="$OUT/studio_small_08_1k.hdr"
mkdir -p "$OUT"
md5of() { if command -v md5sum >/dev/null; then md5sum "$1" | cut -d' ' -f1; else md5 -q "$1"; fi; }
[ -f "$FILE" ] || curl -fsSL "$URL" -o "$FILE"
if [ "$(md5of "$FILE")" != "$MD5" ]; then echo "checksum mismatch: $FILE" >&2; rm -f "$FILE"; exit 1; fi
echo "$FILE"
