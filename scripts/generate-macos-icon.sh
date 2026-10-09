#!/bin/sh
set -eu

project_dir=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
icon_tmp=$(mktemp -d)
trap 'rm -rf "$icon_tmp"' EXIT HUP INT TERM
mkdir "$icon_tmp/foam.iconset"

for size in 16 32 128 256 512; do
    sips -z "$size" "$size" "$project_dir/icon.png" \
        --out "$icon_tmp/foam.iconset/icon_${size}x${size}.png" >/dev/null
    retina_size=$((size * 2))
    sips -z "$retina_size" "$retina_size" "$project_dir/icon.png" \
        --out "$icon_tmp/foam.iconset/icon_${size}x${size}@2x.png" >/dev/null
done

iconutil --convert icns "$icon_tmp/foam.iconset" --output "$project_dir/icon.icns"
