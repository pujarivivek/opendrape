#!/usr/bin/env bash
# Downloads the CC0 MakeHuman data files OpenDrape's bodies are built from, pinned to
# makehuman v1.3.0, and checks every file's SHA-256. Only data (mesh + targets), never code.
# License: makehuman LICENSE.md section C, "These assets have been released under CC0 1.0 Universal."
set -euo pipefail
cd "$(dirname "$0")/.."
BASE=https://raw.githubusercontent.com/makehumancommunity/makehuman/1f508f6083b2f823dab15de924b3bde72e08d77c/makehuman/data
OUT=target/makehuman
mkdir -p "$OUT"
sha256() { if command -v sha256sum >/dev/null; then sha256sum "$1" | cut -d' ' -f1; else shasum -a 256 "$1" | cut -d' ' -f1; fi; }
while read -r sum path; do
  file="$OUT/$(basename "$path")"
  [ -f "$file" ] || curl -fsSL "$BASE/$path" -o "$file"
  if [ "$(sha256 "$file")" != "$sum" ]; then echo "checksum mismatch: $path" >&2; rm -f "$file"; exit 1; fi
done <<'LIST'
8e761e6624b8f54536409135d1636da63b32486a90d4897f84e121d144f6fb4c 3dobjs/base.obj
92d61eeb3c164b421fd5a7c3537ee45e7e1a51de4d49bf312e19c3df2be1d8fc targets/macrodetails/african-female-young.target
095fe79694fa19e1fe98d93009ec116199bd524e081c640351a10eccf2cca1eb targets/macrodetails/asian-female-young.target
118379f6e8ba9266247fdb8788a20e1df40a239f97ced0b9905bcbcc74f6e820 targets/macrodetails/caucasian-female-young.target
LIST
echo "$OUT"
