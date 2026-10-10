#!/bin/sh
# Builds, renders and exports the dress forms with Blender, headless (no window opens).
#
#   sh scripts/forms/blender.sh loft    <form-id>...   build target/forms/build/<id>.blend
#   sh scripts/forms/blender.sh render  <form-id>...   review sheets target/forms/build/<id>.png
#   sh scripts/forms/blender.sh export  <form-id>...   assets/forms/<id>.form.json
#
# Form ids: women-torso, men-torso. Set BLENDER to use another Blender than the Mac default.
# --factory-startup keeps the user's add-ons (some go online) from loading.
set -e
BLENDER=${BLENDER:-/Applications/Blender.app/Contents/MacOS/Blender}
ROOT=$(cd "$(dirname "$0")/../.." && pwd)
BUILD="$ROOT/target/forms/build"
mkdir -p "$BUILD" "$ROOT/assets/forms"
step=$1
shift
for id in "$@"; do
  case $step in
    loft)
      "$BLENDER" -b --factory-startup --python-exit-code 1 \
        --python "$ROOT/scripts/forms/loft_base.py" -- "$id" "$BUILD/$id.blend" ;;
    render)
      "$BLENDER" -b --factory-startup "$BUILD/$id.blend" --python-exit-code 1 \
        --python "$ROOT/scripts/forms/render_views.py" -- "$BUILD/$id.png" "$ROOT/assets/forms/$id.form.json" ;;
    export)
      "$BLENDER" -b --factory-startup "$BUILD/$id.blend" --python-exit-code 1 \
        --python "$ROOT/scripts/forms/export_form.py" -- "$ROOT/assets-src/forms/$id.meta.json" \
        "$ROOT/assets/forms/$id.form.json" ;;
    *)
      echo "usage: sh scripts/forms/blender.sh loft|render|export <form-id>..." >&2
      exit 2 ;;
  esac
done
