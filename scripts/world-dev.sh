#!/usr/bin/env bash
# Generate the small dev world: ~7.6 km² (2.75 km square) around 63.83976, 20.31514 in Umeå, into
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

# 2816 m = eleven 256 m tiles. The ground is sampled every 1 m (what is simulated, and what the
# client's simplified mesh and relief map follow; 256 / 1 = 256 cells a tile). Override with --cell.
exec scripts/world.sh --center 63.83976,20.31514 --half 1408 --cell 1 --out client/public/dev/world.msgpack "$@"
