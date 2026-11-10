---
name: add-orienteering-map
description: Put an orienteering map (OCAD .ocd, and optionally a georeferenced PNG/JPEG render of it) into a Ragnarok world: check the map, name it in the world's area.json, place it, lay the render over the minimap, rebuild the world and deploy it. Use when the user drops a map into temp-maps/ocad/ and asks to use it for a world's terrain, paths, rocks or minimap.
---

# Adding an orienteering map to a world

What the map does to the world, and every `area.json` option, is in [docs/WORLD.md](../../../docs/WORLD.md#orienteering-maps); the symbols are in [docs/ORIENTEERING.md](../../../docs/ORIENTEERING.md). This is the order to do it in. Maps belong to their clubs: they stay in the git-ignored `temp-maps/ocad/`, and deploying one (or its render) needs the mapmaker's permission.

## 1. Look at what you were given

```sh
ls -la temp-maps/ocad/
cargo run -p osm-import --bin ocd-dump -- "temp-maps/ocad/<map>.ocd"
```

- `ocd-dump` must report SWEREF 99 TM (grid code 13002) and ISOM 2017, or the importer refuses the map. It also prints the tiles the map covers: the world needs those, and a margin.
- **A render for the minimap** is a `.png` or `.jpg` with the same name. With a world file beside it (`.pgw`, `.jgw` or `.wld`, plus a `.prj` that must say SWEREF99 TM) it is placed exactly by that. Without one it must be a plain OCAD export of exactly the map's objects, edge to edge. Check the `.pgw`'s pixel size (its first line, metres a pixel) and the image size.
- A PNG render usually leaves the forest see-through; the image viewer shows that as black. That is expected: the importer paints it white inside the map's outline.

## 2. Find the world

```sh
grep -l ocad worlds/*/area.json
for f in worlds/*/area.json; do grep -Ho '"\(ocad\|minimap_image\|ocad_shift\)":[^,}]*' "$f"; done
```

A world that already names the `.ocd` only needs `minimap_image`. A new world is painted in the map editor (`cargo run -p editor`, see [docs/EDITOR.md](../../../docs/EDITOR.md)) over the tiles `ocd-dump` printed; ask the user which world if it is not obvious.

## 3. Name the map in `area.json`

The editor writes `area.json` on one line and keeps keys it does not know, so add the keys by text, not by reformatting the file:

```json
"ocad":"temp-maps/ocad/<map>.ocd","minimap_image":"temp-maps/ocad/<map>.png"
```

Add `"cell": 1` if the knolls and pits matter (at 2 or 4 m they fall between the ground samples). Check the file still parses (`python3 -c 'import json,sys; json.load(open(sys.argv[1]))' worlds/<name>/area.json`).

## 4. Build it somewhere safe first

The import takes about a minute when `.cache/osm-import` is warm. Build into the scratchpad, so the world the user's local server runs is not swapped under it:

```sh
set -a; . ./.env.local; set +a   # Lantmäteriet credentials
cargo run --release -p osm-import -- --area worlds/<name>/area.json --out <scratch>/world.msgpack
```

In the log, check:

- **No `no orienteering map:` or `no print of the map:` line.** Either means that part was skipped; the error says why.
- **`median offset to them … m east, … m south`.** A metre or two is fine. More, and `ocad_shift: [east, south]` in `area.json` moves the map; build again.
- **`[minimap: …]`** ran, and `minimap.webp` is next to the output. With a print it is 4096 px and a megabyte or two.

Then look at `<scratch>/minimap.webp` with the Read tool. The map should sit where the roads and railway of the drawn map run into it. The outline is a convex hull, so unmapped corners inside it show as white forest; say so if they are large.

## 5. Put the world in place

Either rebuild into the world itself (the same command with `--out worlds/<name>/world.msgpack`, or **Generate world** in the editor), or, when only the minimap changed, copy `<scratch>/minimap.webp` into `worlds/<name>/`. The client loads `minimap.webp` before an older `minimap.png`. A local server needs a restart to pick up a rebuilt world.

## 6. Commit and deploy

- Commit `area.json` (the generated files and `temp-maps/` are git-ignored).
- Deploy that world as the live one, following AGENTS.md's "Deploying": `WORLD_DIR=worlds/<name> DOMAIN="$TARGET_DOMAIN" scripts/deploy.sh "$SSH_USER@$SSH_HOST"`. The deploy ships `minimap.webp`.
- Check it: `ssh "$SSH_USER@$SSH_HOST" 'docker ps --filter name=^ragnarok$; docker exec ragnarok ls -la /app/dist/ | grep minimap'`.
