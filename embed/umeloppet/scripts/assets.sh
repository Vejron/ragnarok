#!/bin/sh
# Gathers everything the club's site has to host into one folder: the built element, the world and its
# course, and the few models and sounds the embed uses. Upload the folder as it is; the element finds the
# rest next to its own script.
#
#   npm run build && sh scripts/assets.sh ../../worlds/umeloppet upload/
set -eu

world=${1:?usage: sh scripts/assets.sh <world dir> <out dir>}
out=${2:?usage: sh scripts/assets.sh <world dir> <out dir>}
here=$(cd "$(dirname "$0")/.." && pwd)
public="$here/../../client/public"

[ -f "$here/dist/umeloppet.js" ] || { echo "no dist/umeloppet.js: run npm run build first" >&2; exit 1; }
[ -f "$world/world.msgpack" ] || { echo "$world has no world.msgpack: build the world first" >&2; exit 1; }
[ -f "$world/course.json" ] || { echo "$world has no course.json: run npm run course first" >&2; exit 1; }

rm -rf "$out"
mkdir -p "$out/models" "$out/sfx"
cp -R "$here/dist/." "$out/"
cp "$world/world.msgpack" "$world/course.json" "$out/"
cp -RL "$world/tiles" "$out/tiles"
for map in minimap.webp minimap.png; do
  if [ -f "$world/$map" ]; then cp "$world/$map" "$out/"; break; fi
done
# The skier and the snowmobile; the credits go with them.
cp "$public/models/player.glb" "$public/models/snowmobile.glb" "$public/models/CREDITS.md" "$out/models/"
# The sound bank loads all of its samples, so all of them go.
cp -R "$public/sfx/." "$out/sfx/"

echo "$out: $(du -sh "$out" | cut -f1) in $(find "$out" -type f | wc -l | tr -d ' ') files"
