#!/usr/bin/env bash
# Generate the small dev world: ~5 km² (2.25 km square) around Mariehem, Umeå, into
# client/public/dev/ so the big world in client/public/ is left alone. Much faster to import,
# load and iterate on. Extra arguments go to the importer (a later --center/--half wins).
#   scripts/world-dev.sh                 real data; needs LM_USER and LM_PASS in .env.local
#   scripts/world-dev.sh --synthetic     offline procedural city, no credentials
# Use it:
#   RAGNAROK_WORLD=client/public/dev/world.msgpack cargo run -p server
#   cd client && VITE_WORLD_BASE=/dev/ npm run dev
# Shares .cache/osm-import with the big world; nothing there is evicted.
set -euo pipefail
cd "$(dirname "$0")/.."

# 2250 m = nine 250 m tiles. Mariehem is the OSM suburb node (63.8372, 20.3261).
exec scripts/world.sh --center 63.8372,20.3261 --half 1125 --out client/public/dev/world.msgpack "$@"
