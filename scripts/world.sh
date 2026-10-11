#!/usr/bin/env bash
# Generate the world the server and client load (client/public/world.msgpack + tiles/).
# The area is the importer's default (central Umeå, Backen to Nydala); extra arguments go to it.
#   scripts/world.sh                 real data; needs LM_USER and LM_PASS in .env.local
#   scripts/world.sh --synthetic     offline procedural city, no credentials
# Downloads are cached in .cache/osm-import: keep it, it is what makes a regeneration identical.
set -euo pipefail
cd "$(dirname "$0")/.."

if [[ -f .env.local ]]; then
  set -a; . ./.env.local; set +a
fi

synthetic=0
for arg in "$@"; do [[ "$arg" == "--synthetic" ]] && synthetic=1; done
if [[ $synthetic == 0 && ( -z "${LM_USER:-}" || -z "${LM_PASS:-}" ) ]]; then
  echo "LM_USER and LM_PASS are not set (put them in .env.local); use --synthetic for an offline world" >&2
  exit 1
fi

exec cargo run --release -p osm-import -- "$@"
