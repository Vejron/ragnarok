//! Builds the world from OpenStreetMap: buildings and roads inside a bounding box, projected to
//! local metres, cut into square tiles with their collision meshes, plus the manifest with the
//! spawn points. Writes `world.msgpack` and `tiles/{col}_{row}.msgpack` next to it.
//!
//!     cargo run -p osm-import                                  # Backen, Umeå, 2 km square
//!     cargo run -p osm-import -- --center 59.3167,18.0613 --half 300
//!     cargo run -p osm-import -- --bbox 59.3140,18.0560,59.3195,18.0665
//!     cargo run -p osm-import -- --synthetic                   # offline grid city, with hills
//!     cargo run -p osm-import -- --dem m708_75.tif             # elevation from a local DTM GeoTIFF
//!     LM_USER=… LM_PASS=… cargo run -p osm-import              # elevation fetched from Lantmäteriet
//!     cargo run -p osm-import -- --cell 0                      # flat ground, no elevation
//!     cargo run -p osm-import -- --no-water                    # skip Lantmäteriet's Hydrografi
//!     cargo run -p osm-import -- --no-minimap                  # skip the pre-rendered map image
//!     cargo run -p osm-import -- --no-trees                    # skip Umeå's tree register
//!     cargo run -p osm-import -- --refresh                     # download everything again
//!     cargo run -p osm-import -- --clear-cache                 # delete .cache/osm-import
//!     cargo run -p osm-import -- --bake worlds/x/world.msgpack  # bake the cubes into an older world
//!
//! Credentials, data sources and the cache are described in `WORLD.md`.

use osm_import::region::{Place, Region};
use osm_import::{assemble, bushes, cache, canopy, dem, detail, grid, ground, hydro, ign, lidar, minimap, overpass, paint, progress, roof, trees, voxels, water};
use anyhow::{Context, Result, bail};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Generate a Ragnarok world file from OpenStreetMap")]
struct Args {
    /// lat,lon of the world's centre, in degrees. Default: central Umeå, so that with the default
    /// `--half` the world reaches from Backen in the west to Nydala in the east.
    #[arg(long, default_value = "63.8225,20.235")]
    center: String,
    /// Half the side of the square around `--center`, metres.
    #[arg(long, default_value_t = 4300.0)]
    half: f64,
    /// south,west,north,east in degrees. Overrides `--center` and `--half`.
    #[arg(long)]
    bbox: Option<String>,
    /// A map editor world (`worlds/<name>/area.json`): build exactly its painted tiles of the global
    /// grid (`grid::TILE`), with the ground sampled at the file's `cell` unless `--cell` is given.
    /// Overrides `--center`, `--half` and `--bbox`; `--tile` stays the grid's.
    #[arg(long)]
    area: Option<PathBuf>,
    /// Side of a tile, metres.
    #[arg(long, default_value_t = grid::TILE)]
    tile: f64,
    /// Distance between terrain height samples, metres; must divide `--tile`. 0 makes the ground
    /// flat. Default `DEFAULT_CELL`, or an `--area` world's own.
    #[arg(long = "cell")]
    cell_arg: Option<f64>,
    /// `--cell` as given, else the area's, else `DEFAULT_CELL`.
    #[arg(skip)]
    cell: f64,
    /// Spacing of the slope map baked into each tile for the client's lighting, metres; must
    /// divide `--tile`. Finer than `--cell`, so a coarse ground can still be shaded by the 1 m height
    /// model. 0 leaves it out.
    #[arg(long, default_value_t = 0.0)]
    detail: f64,
    /// A Lantmäteriet DTM GeoTIFF (SWEREF 99 TM) to take elevation from. Without it, the DTM is
    /// fetched with the credentials in LM_USER and LM_PASS.
    #[arg(long)]
    dem: Option<PathBuf>,
    /// Leave buildings as boxes of their tagged height instead of fitting roofs to the laser data.
    #[arg(long)]
    no_roofs: bool,
    /// France: leave the buildings' colours to the client instead of taking the roofs' from IGN's
    /// aerial photo.
    #[arg(long)]
    no_paint: bool,
    /// Leave out lakes, rivers and streams.
    #[arg(long)]
    no_water: bool,
    /// Take lakes, rivers, streams and the sea from OpenStreetMap instead of Lantmäteriet's
    /// Hydrografi Direkt. Slower (Overpass), but it has the sea.
    #[arg(long)]
    osm_water: bool,
    /// Do not render `minimap.png`, the top-down map the client shows, next to the world.
    #[arg(long)]
    no_minimap: bool,
    /// Leave out the trees from Umeå's open data.
    #[arg(long)]
    no_trees: bool,
    /// Leave out the trees found in the laser data (only the register's remain).
    #[arg(long)]
    no_lidar_trees: bool,
    /// Shortest tree to take from the laser data, metres above the terrain.
    #[arg(long, default_value_t = canopy::MIN_HEIGHT)]
    min_tree_height: f64,
    /// Bake the buildings' cubes into a world written before they were (world version 10), in
    /// place: its manifest, and the tiles next to it. Nothing is downloaded.
    #[arg(long)]
    bake: Option<PathBuf>,
    /// Skip OpenStreetMap and generate a procedural grid city (works offline).
    #[arg(long)]
    synthetic: bool,
    /// Overpass API endpoints, tried in order (the public servers are often busy).
    #[arg(long, value_delimiter = ',', default_value = "https://overpass-api.de/api/interpreter,https://overpass.kumi.systems/api/interpreter,https://overpass.private.coffee/api/interpreter,https://maps.mail.ru/osm/tools/overpass/api/interpreter")]
    overpass: Vec<String>,
    /// Overpass is asked for squares of about this many metres and the answers merged, so a big
    /// box does not time out. Finished squares are cached.
    #[arg(long, default_value_t = 2000.0)]
    overpass_chunk: f64,
    #[arg(long, default_value = "client/public/world.msgpack")]
    out: PathBuf,
    /// Where downloads are kept between runs (git-ignored). See WORLD.md.
    #[arg(long, default_value = ".cache/osm-import")]
    cache: PathBuf,
    /// Do not read or write the download cache.
    #[arg(long)]
    no_cache: bool,
    /// Download everything again and replace what the cache holds.
    #[arg(long)]
    refresh: bool,
    /// Delete the download cache and exit.
    #[arg(long)]
    clear_cache: bool,
    /// Print a `@progress {json}` line on stderr as the stage, its steps and the download rate
    /// change. The map editor runs the importer with this.
    #[arg(long)]
    progress: bool,
}

fn parse_bbox(text: &str) -> Result<[f64; 4]> {
    let values: Vec<f64> = text.split(',').map(|part| part.trim().parse::<f64>()).collect::<Result<_, _>>().context("bbox must be four numbers")?;
    let [south, west, north, east] = values[..] else { bail!("bbox must be south,west,north,east") };
    if !(south < north && west < east) {
        bail!("bbox must be south,west,north,east with south < north and west < east");
    }
    Ok([south, west, north, east])
}

fn parse_center(text: &str) -> Result<(f64, f64)> {
    let values: Vec<f64> = text.split(',').map(|part| part.trim().parse::<f64>()).collect::<Result<_, _>>().context("center must be two numbers")?;
    let [lat, lon] = values[..] else { bail!("center must be lat,lon") };
    Ok((lat, lon))
}

/// The window `[e0, n0, e1, n1]` over the world's whole grid of tiles, in the region's projected
/// coordinates (SWEREF 99 TM, Lambert-93).
fn source_window(args: &Args, place: &Place) -> dem::Window {
    // The square of tiles, which may stick out of the playable square.
    let reach = (2.0 * place.half / args.tile).ceil() * args.tile / 2.0 + args.cell;
    let corners = [(-reach, -reach), (reach, -reach), (-reach, reach), (reach, reach)].map(|(x, z)| place.source(x, z));
    let fold = |init: f64, pick: fn(f64, f64) -> f64, axis: fn(&(f64, f64)) -> f64| corners.iter().map(axis).fold(init, pick);
    [
        fold(f64::MAX, f64::min, |c| c.0),
        fold(f64::MAX, f64::min, |c| c.1),
        fold(f64::MIN, f64::max, |c| c.0),
        fold(f64::MIN, f64::max, |c| c.1),
    ]
}

/// The painted tiles as rectangles of the region's projected coordinates, each grown by
/// `CANOPY_REACH` so a crown or a roof just over a tile's edge is still read. Sweden's tile grid is
/// not aligned with SWEREF's, so each is the box around its four projected corners.
fn painted_rects(args: &Args, place: &Place, mask: &std::collections::BTreeSet<(u16, u16)>) -> Vec<dem::Window> {
    const CANOPY_REACH: f64 = 30.0;
    let half = place.half;
    mask.iter()
        .map(|&(col, row)| {
            let (x0, z0) = (-half + f64::from(col) * args.tile, -half + f64::from(row) * args.tile);
            let corners = [(x0, z0), (x0 + args.tile, z0), (x0, z0 + args.tile), (x0 + args.tile, z0 + args.tile)].map(|(x, z)| place.source(x, z));
            let low = |axis: fn(&(f64, f64)) -> f64| corners.iter().map(axis).fold(f64::MAX, f64::min);
            let high = |axis: fn(&(f64, f64)) -> f64| corners.iter().map(axis).fold(f64::MIN, f64::max);
            [low(|c| c.0) - CANOPY_REACH, low(|c| c.1) - CANOPY_REACH, high(|c| c.0) + CANOPY_REACH, high(|c| c.1) + CANOPY_REACH]
        })
        .collect()
}

/// Lakes, rivers and streams in local metres: from Lantmäteriet's Hydrografi in Sweden, from
/// OpenStreetMap with `--osm-water` and always in France. Empty with `--no-water`, without
/// credentials or on flat ground (the surface height comes from the DTM).
fn water_map(args: &Args, place: &Place, pieces: Option<&[[f64; 4]]>) -> Result<water::Map> {
    if args.no_water || args.cell == 0.0 {
        return Ok(water::Map::default());
    }
    let frame = place.frame;
    // The whole grid of tiles, which may stick out of the playable square.
    let reach = (2.0 * place.half / args.tile).ceil() * args.tile / 2.0 + args.cell;
    let [south, west, north, east] = match place.region {
        // Exactly the two corners, as always, so the cached answers are found again.
        Region::Sweden => {
            let (south, west) = frame.unproject(-reach, reach);
            let (north, east) = frame.unproject(reach, -reach);
            [south, west, north, east]
        }
        Region::France => osm_import::region::enclosing(frame, reach),
    };
    let local = |ring: &[overpass::LatLon]| -> Vec<[f64; 2]> { ring.iter().map(|p| frame.project(p.lat, p.lon)).collect() };
    // Rings are closed in GeoJSON: drop the repeated first point.
    let open = |mut ring: Vec<[f64; 2]>| {
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        ring
    };
    let mut map = water::Map::default();
    let polygons = |found: &[osm_import::osm_water::Polygon], kind, map: &mut water::Map| {
        for polygon in found {
            map.bodies.push(water::Body { kind, rings: polygon.iter().map(|ring| open(local(ring))).filter(|ring| ring.len() >= 3).collect(), level: None });
        }
    };
    if !args.osm_water && place.region == Region::Sweden {
        let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
            eprintln!("no LM_USER / LM_PASS: the world has no water (or pass --osm-water)");
            return Ok(map);
        };
        let found = match pieces {
            Some(pieces) => hydro::fetch_pieces(pieces, &user, &password),
            None => hydro::fetch([south, west, north, east], &user, &password),
        }
        .context("Hydrografi failed; retry later, use --osm-water or --no-water")?;
        polygons(&found.lakes, water::Kind::Lake, &mut map);
        polygons(&found.rivers, water::Kind::River, &mut map);
        map.streams = found.streams.iter().map(|line| local(line)).collect();
        eprintln!("water: {} lakes, {} river polygons, {} stream lines from Hydrografi", found.lakes.len(), found.rivers.len(), found.streams.len());
        return Ok(map);
    }
    let found = match pieces {
        Some(pieces) => osm_import::osm_water::fetch_pieces(&args.overpass, pieces, 3, 15),
        None => osm_import::osm_water::fetch_pieces(&args.overpass, &overpass::split([south, west, north, east], args.overpass_chunk), 3, 15),
    }
    .context("OpenStreetMap water failed; retry later, use Hydrografi or --no-water")?;
    polygons(&found.lakes, water::Kind::Lake, &mut map);
    polygons(&found.rivers, water::Kind::River, &mut map);
    map.streams = found.streams.iter().map(|line| local(line)).collect();
    // The sea: the coastline in [east, north] metres, closed along the edge of the grid.
    let coast: Vec<Vec<[f64; 2]>> = found.coast.iter().map(|chain| local(chain).into_iter().map(|[x, z]| [x, -z]).collect()).collect();
    let seas = osm_import::osm_water::sea(&coast, reach);
    for rings in &seas {
        let rings = rings.iter().map(|ring| open(ring.iter().map(|&[x, y]| [x, -y]).collect())).collect();
        map.bodies.push(water::Body { kind: water::Kind::Lake, rings, level: Some(0.0) });
    }
    eprintln!("water: {} lakes, {} river polygons, {} stream lines, {} sea polygons from OpenStreetMap", found.lakes.len(), found.rivers.len(), found.streams.len(), seas.len());
    Ok(map)
}

/// The terrain under the world, the elevation of its y = 0 and the water over it, from `--dem` or
/// Lantmäteriet, or flat with `--cell 0`. Lakes and rivers are carved into the ground.
fn terrain(args: &Args, place: &Place, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, map: &water::Map) -> Result<(shared::terrain::Ground, f64, Option<dem::Dem>, water::Placed)> {
    if args.cell == 0.0 {
        return Ok((Default::default(), 0.0, None, Default::default()));
    }
    let (bbox, half) = (place.bbox, place.half);
    let window = source_window(args, place);
    progress::stage("elevation");
    let source = match &args.dem {
        Some(path) => dem::Dem::from_path(path, window)?,
        // IGN's 0.5 m MNT, only under the painted tiles when there are some.
        None if place.region == Region::France => ign::mnt(&mask.map_or_else(|| vec![window], |mask| painted_rects(args, place, mask))).context("IGN's MNT failed; retry later, or pass --dem <file.tif>")?,
        None => {
            let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
                bail!("elevation needs --dem <file.tif>, or LM_USER and LM_PASS (Lantmäteriet credentials), or --cell 0 for flat ground");
            };
            dem::Dem::from_stac(bbox, window, &user, &password)?
        }
    };
    let half_cell = args.cell / 2.0;
    let dtm = |x: f64, z: f64| {
        let (e, n) = place.source(x, z);
        source.mean(e, n, half_cell).map(f64::from)
    };
    progress::stage("ground");
    let surface = water::Surface::new(map, ground::grid(half, args.tile, args.cell)?, dtm);
    // Samples on or inside a painted tile; everywhere else is not read, so it costs no download.
    let (tile_cm, half_cm) = ((args.tile * 100.0).round() as i64, (half * 100.0).round() as i64);
    let wanted = |x: f64, z: f64| {
        let Some(mask) = mask else { return true };
        let (xc, zc) = (((x * 100.0).round() as i64 + half_cm), ((z * 100.0).round() as i64 + half_cm));
        let spans = |v: i64| {
            let c = v.div_euclid(tile_cm);
            // On a border a sample belongs to both tiles.
            if v.rem_euclid(tile_cm) == 0 { vec![c - 1, c] } else { vec![c] }
        };
        spans(xc).iter().any(|&c| spans(zc).iter().any(|&r| c >= 0 && r >= 0 && mask.contains(&(c as u16, r as u16))))
    };
    let (ground, datum) = ground::build_where(half, args.tile, args.cell, wanted, dtm, |x, z, dem| surface.bed(x, z, dem))?;
    let placed = water::Placed::new(map, &surface, datum, &ground);
    report(map, &placed);
    Ok((ground, datum, Some(source), placed))
}

fn report(map: &water::Map, placed: &water::Placed) {
    let wet = placed.levels.iter().filter(|&&level| level != shared::world::NO_WATER).count();
    eprintln!("water: {} bodies, {} samples under or beside water, {} stream lines kept", map.bodies.len(), wet, placed.streams.len());
}

/// What the laser data gives. Roofs, decks and trees are each empty when switched off, or without
/// credentials (`--no-roofs`, `--no-lidar-trees`).
struct Laser {
    /// The returns that can be roof.
    roofs: Option<roof::Cloud>,
    /// Bridge deck returns, from a classified cloud only.
    decks: Option<roof::Cloud>,
    trees: Vec<canopy::Detected>,
    /// From a classified cloud's medium vegetation only.
    bushes: Vec<bushes::Bush>,
}

fn laser(args: &Args, place: &Place, mask: Option<&std::collections::BTreeSet<(u16, u16)>>, datum: f64, osm: &overpass::Osm, dem: Option<&dem::Dem>) -> Result<Laser> {
    let none = || Laser { roofs: None, decks: None, trees: Vec::new(), bushes: Vec::new() };
    let want_roofs = !args.no_roofs;
    let want_trees = !args.no_lidar_trees && dem.is_some();
    if args.cell == 0.0 || !(want_roofs || want_trees) {
        return Ok(none());
    }
    let bbox = place.bbox;
    let credentials = match place.region {
        // IGN's LiDAR HD is open.
        Region::France => (String::new(), String::new()),
        Region::Sweden => {
            let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
                eprintln!("no LM_USER / LM_PASS: buildings stay boxes and only the register's trees are planted");
                return Ok(none());
            };
            (user, password)
        }
    };
    let window = source_window(args, place);
    let mut cloud = want_roofs.then(|| roof::Cloud::new(window));
    let mut canopy = want_trees.then(|| canopy::Canopy::new(window));
    let mut decks = (want_roofs && place.region == Region::France).then(|| roof::Cloud::new(window));
    let mut undergrowth = (want_trees && place.region == Region::France).then(|| bushes::Undergrowth::new(window));
    let painted = mask.map(|mask| painted_rects(args, place, mask));
    progress::stage("laser");
    let total = match place.region {
        Region::Sweden => lidar::fetch(bbox, window, painted.as_deref(), &credentials.0, &credentials.1, |p| {
            if let Some(cloud) = &mut cloud {
                cloud.add(&p, datum);
            }
            if let Some(canopy) = &mut canopy {
                canopy.add(&p);
            }
        })?,
        // Classified: roofs read only building returns, and trees only high vegetation, so
        // neither has to tell foliage from roofs by shape.
        Region::France => {
            let squares: Vec<ign::Square> = ign::squares(window)?
                .into_iter()
                .filter(|square| painted.as_ref().is_none_or(|rects| rects.iter().any(|r| square.window[0] < r[2] && r[0] < square.window[2] && square.window[1] < r[3] && r[1] < square.window[3])))
                .collect();
            eprintln!("laser: {} LiDAR HD squares", squares.len());
            let client = reqwest::blocking::Client::builder().build()?;
            lidar::read_files(&client, &ign::clouds(&squares), window, painted.as_deref(), ("", ""), |p| match p.class {
                ign::class::BUILDING => {
                    if let Some(cloud) = &mut cloud {
                        cloud.add(&p, datum);
                    }
                }
                ign::class::BRIDGE => {
                    if let Some(decks) = &mut decks {
                        decks.add(&p, datum);
                    }
                }
                ign::class::HIGH_VEGETATION => {
                    if let Some(canopy) = &mut canopy {
                        canopy.add(&p);
                    }
                    if let Some(undergrowth) = &mut undergrowth {
                        undergrowth.crown(&p);
                    }
                }
                ign::class::MEDIUM_VEGETATION => {
                    if let Some(undergrowth) = &mut undergrowth {
                        undergrowth.add(&p);
                    }
                }
                _ => {}
            })?
        }
    };
    eprintln!("laser: {total} returns{}", cloud.as_ref().map_or(String::new(), |c| format!(", {} kept for roofs", c.len())));
    let mut detected = Vec::new();
    let mut found_bushes = Vec::new();
    if let (Some(undergrowth), Some(dem)) = (undergrowth, dem) {
        found_bushes = undergrowth.detect(|e, n| dem.mean(e, n, 0.5));
        eprintln!("undergrowth: {} bushes", found_bushes.len());
    }
    if let (Some(mut canopy), Some(dem)) = (canopy, dem) {
        // Roofs are not trees: keep the canopy off every building outline. A classified cloud's
        // vegetation has no roofs in it, and masking would only cut crowns that overhang one.
        for way in osm.ways.iter().filter(|way| place.region == Region::Sweden && way.tags.contains_key("building")) {
            let ring: Vec<(f64, f64)> = way.geometry.iter().map(|p| place.region.source(p.lat, p.lon)).collect();
            canopy.mask(&ring, 1.5);
        }
        progress::stage("canopy");
        detected = canopy.detect(|e, n| dem.mean(e, n, 0.5), args.min_tree_height);
        eprintln!("canopy: {} trees at least {} m tall", detected.len(), args.min_tree_height);
    }
    Ok(Laser { roofs: cloud, decks, trees: detected, bushes: found_bushes })
}

/// The aerial photo the roofs' colours come from: France only, and none if IGN does not answer
/// (the colours are a nicety; the client picks its own without them).
fn photo(args: &Args, place: &Place, mask: Option<&std::collections::BTreeSet<(u16, u16)>>) -> Option<paint::Photo> {
    if args.no_paint || place.region != Region::France {
        return None;
    }
    progress::stage("aerial photo");
    let rects = mask.map_or_else(|| vec![source_window(args, place)], |mask| painted_rects(args, place, mask));
    ign::photo(&rects).inspect_err(|error| eprintln!("no roof colours: {error:#}")).ok()
}

/// `--bake`: reads the world at `path`, bakes every tile (`voxels::bake`), gives the cubes houses
/// share to one of them (`voxels::share`) and writes it back as the current world version, with the
/// tiles' new hashes. A world baked already is only shared again, which changes nothing once done.
fn bake_world(path: &std::path::Path) -> Result<()> {
    use shared::world::{Manifest, TileRef, WORLD_VERSION, Tile, world_hash};
    let bytes = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
    let mut manifest = match Manifest::decode(&bytes) {
        Ok(manifest) => manifest,
        // Before version 15 the manifest held the origin where the frame is now: every such world is
        // Swedish, so its frame is the equirectangular one around that origin.
        Err(error) => {
            #[derive(serde::Deserialize)]
            struct Old(u8, (f64, f64), i32, i32, Vec<[i32; 2]>, Vec<TileRef>, i32);
            let Old(version, origin, half_extent, tile_size, spawns, tiles, ground_cell) =
                rmp_serde::from_slice(&bytes).map_err(|_| error).with_context(|| format!("decoding {}", path.display()))?;
            Manifest { version, frame: shared::world::Frame::Equirect { origin }, half_extent, tile_size, spawns, tiles, ground_cell }
        }
    };
    anyhow::ensure!(
        manifest.version + 1 >= WORLD_VERSION && manifest.version <= WORLD_VERSION,
        "{} is world version {}; only {} and {} can be baked",
        path.display(),
        manifest.version,
        WORLD_VERSION - 1,
        WORLD_VERSION
    );
    let dir = path.parent().map(PathBuf::from).unwrap_or_default();
    let mut tiles = Vec::new();
    let mut before = 0;
    for entry in &manifest.tiles {
        let file = dir.join(entry.path());
        let bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
        before += bytes.len();
        let mut tile = Tile::decode(&bytes).with_context(|| format!("decoding {}", file.display()))?;
        voxels::bake(&mut tile);
        tiles.push(tile);
    }
    let shared = voxels::share(&mut tiles.iter_mut().collect::<Vec<_>>());
    let mut after = 0;
    let mut refs = Vec::new();
    for (entry, tile) in manifest.tiles.iter().zip(&tiles) {
        let file = dir.join(entry.path());
        let baked = tile.encode();
        after += baked.len();
        std::fs::write(&file, &baked).with_context(|| format!("writing {}", file.display()))?;
        refs.push(TileRef { hash: world_hash(&baked), ..*entry });
    }
    manifest.tiles = refs;
    manifest.version = WORLD_VERSION;
    std::fs::write(path, manifest.encode()).with_context(|| format!("writing {}", path.display()))?;
    eprintln!("baked {} tiles: {} KiB before, {} KiB after; {shared} cubes shared by two houses kept by one", manifest.tiles.len(), before / 1024, after / 1024);
    Ok(())
}

/// Distance between terrain height samples when neither `--cell` nor the area says, metres: a
/// power of two, so a 256 m tile is `2^k + 1` samples a side, which the client simplifies best.
const DEFAULT_CELL: f64 = 4.0;

/// The ground resolution an editor world asks for (`cell` in its `area.json`), if any.
fn area_cell(path: &std::path::Path) -> Result<Option<f64>> {
    #[derive(serde::Deserialize)]
    struct File {
        cell: Option<f64>,
    }
    let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
    Ok(serde_json::from_str::<File>(&text).with_context(|| format!("parsing {}", path.display()))?.cell)
}

fn main() -> Result<()> {
    let mut args = Args::parse();
    let from_area = match &args.area {
        Some(path) => area_cell(path)?,
        None => None,
    };
    args.cell = args.cell_arg.or(from_area).unwrap_or(DEFAULT_CELL);
    if let Some(path) = &args.bake {
        return bake_world(path);
    }
    if args.clear_cache {
        let freed = cache::clear(&args.cache)?;
        eprintln!("cleared {} ({} MiB)", args.cache.display(), freed / (1024 * 1024));
        return Ok(());
    }
    cache::init((!args.no_cache).then(|| args.cache.clone()), args.refresh);
    if args.progress {
        progress::events();
    }
    anyhow::ensure!(args.tile >= 20.0, "--tile must be at least 20 m");
    anyhow::ensure!(args.cell == 0.0 || args.cell >= 0.5, "--cell must be 0 or at least 0.5 m");
    let built = if args.synthetic {
        let map = if args.no_water { water::Map::default() } else { assemble::synthetic_water(300.0) };
        let (ground, datum, placed) = if args.cell > 0.0 {
            let surface = water::Surface::new(&map, ground::grid(300.0, args.tile, args.cell)?, |x, z| Some(ground::hills(x, z)));
            let (ground, datum) = ground::build(300.0, args.tile, args.cell, |x, z| Some(ground::hills(x, z)), |x, z, dem| surface.bed(x, z, dem))?;
            let placed = water::Placed::new(&map, &surface, datum, &ground);
            (ground, datum, placed)
        } else {
            Default::default()
        };
        let coarse = ground.clone();
        let mut built = assemble::synthetic_city(300.0, args.tile, ground, placed);
        if args.detail > 0.0 && args.cell > 0.0 {
            // The hills are continuous, so the detail is what the coarse triangles leave out.
            let hills = |x: f64, z: f64| Some(ground::hills(x, z));
            detail::bake(&mut built, &coarse, datum, args.detail, hills, hills);
        }
        built
    } else {
        let area = match &args.area {
            Some(path) => {
                #[derive(serde::Deserialize)]
                struct File {
                    tiles: Vec<grid::Tile>,
                    #[serde(default)]
                    region: Region,
                }
                let text = std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
                let file: File = serde_json::from_str(&text).with_context(|| format!("parsing {}", path.display()))?;
                anyhow::ensure!((args.tile - grid::TILE).abs() < 1e-9, "--area worlds use the global grid's {} m tiles", grid::TILE);
                let layout = grid::layout(file.region, &file.tiles)?;
                eprintln!("area: {} tiles of {}'s grid in a square of {} tiles, {} download blocks", file.tiles.len(), file.region.name(), layout.place.half * 2.0 / grid::TILE, layout.pieces.len());
                Some(layout)
            }
            None => None,
        };
        let mask = area.as_ref().map(|layout| &layout.mask);
        let pieces = area.as_ref().map(|layout| layout.pieces.as_slice());
        let place = match (&area, &args.bbox) {
            (Some(layout), _) => layout.place,
            (None, Some(text)) => {
                let bbox = parse_bbox(text)?;
                Region::at((bbox[0] + bbox[2]) / 2.0, (bbox[1] + bbox[3]) / 2.0).within(bbox)
            }
            (None, None) => {
                let (lat, lon) = parse_center(&args.center)?;
                Region::at(lat, lon).around(lat, lon, args.half)
            }
        };
        let bbox = place.bbox;
        if place.region != Region::Sweden {
            eprintln!("region: {}, frame {:?}", place.region.name(), place.frame);
        }
        // Before the network-flaky step: a world that cannot fit the format should fail in seconds.
        progress::stage("water");
        let map = water_map(&args, &place, pieces)?;
        let (ground, datum, dem, placed) = terrain(&args, &place, mask, &map)?;
        // The public servers shed load with 504s; a few rounds over all of them usually get through.
        progress::stage("overpass");
        let osm = match pieces {
            Some(pieces) => overpass::fetch_pieces(&args.overpass, pieces, 3, 15),
            None => overpass::fetch_chunked(&args.overpass, bbox, args.overpass_chunk, 3, 15),
        }
        .context("Overpass failed; retry later or use --synthetic")?;
        let Laser { roofs: cloud, decks, trees: detected, bushes: found_bushes } = laser(&args, &place, mask, datum, &osm, dem.as_ref())?;
        progress::stage("trees");
        let register = match trees::Register::over(place.region, bbox).filter(|_| !args.no_trees) {
            None => Vec::new(),
            Some(register) => match register.fetch_pieces(pieces.unwrap_or(&[bbox])) {
                Ok(planted) => {
                    eprintln!("trees: {} from {}", planted.len(), register.name());
                    planted
                }
                // The tree data is a nicety; a world without it is still a world.
                Err(error) => {
                    eprintln!("no trees: {error:#}");
                    Vec::new()
                }
            }
        };
        let mut planted = trees::merge(place.region, register, &detected);
        eprintln!("trees: {} from the register, {} from the laser", planted.iter().filter(|t| t.registered).count(), planted.iter().filter(|t| !t.registered).count());
        planted.extend(found_bushes.iter().map(|bush| {
            let (lat, lon) = place.region.degrees(bush.e, bush.n);
            trees::Planted { lat, lon, kind: shared::world::TREE_BUSH, height: bush.height, crown: bush.crown, registered: false }
        }));
        let coarse = (args.detail > 0.0).then(|| ground.clone());
        let photo = photo(&args, &place, mask);
        progress::stage("assemble");
        let mut built = assemble::from_osm(&place, args.tile, mask, &osm, &planted, ground, placed, cloud.as_ref(), decks.as_ref(), photo.as_ref());
        if let (Some(coarse), Some(source)) = (coarse, dem.as_ref()) {
            progress::stage("detail");
            let model = |half: f64| {
                let place = &place;
                move |x: f64, z: f64| {
                    let (e, n) = place.source(x, z);
                    source.mean(e, n, half).map(f64::from)
                }
            };
            detail::bake(&mut built, &coarse, datum, args.detail, model(args.cell / 2.0), model(args.detail / 2.0));
        }
        built
    };
    let (buildings, roads, triangles) = built.tiles.values().fold((0, 0, 0), |(b, r, t), tile| (b + tile.buildings.len(), r + tile.roads.len(), t + tile.collision.triangles.len()));
    let cover: usize = built.tiles.values().map(|tile| tile.cover.len()).sum();
    let rails: usize = built.tiles.values().map(|tile| tile.rails.len()).sum();
    let mut by_kind = [0usize; 10];
    for tree in built.tiles.values().flat_map(|tile| &tile.trees) {
        by_kind[usize::from(tree.kind).min(9)] += 1;
    }
    eprintln!(
        "trees: {} birch, {} pine, {} spruce, {} ornamental, {} broadleaf, {} columnar, {} low-wide; {} rocks, {} bushes",
        by_kind[0], by_kind[1], by_kind[2], by_kind[3], by_kind[4], by_kind[5], by_kind[6], by_kind[7] + by_kind[8], by_kind[9]
    );
    let mut roofs = [0usize; 4];
    for building in built.tiles.values().flat_map(|tile| &tile.buildings) {
        roofs[usize::from(building.roof_kind).min(3)] += 1;
    }
    eprintln!("roofs: {} flat, {} shed, {} gable, {} hip", roofs[0], roofs[1], roofs[2], roofs[3]);
    let (spawns, tile_count) = (built.manifest.spawns.len(), built.tiles.len());
    progress::stage("minimap");
    let map = if args.no_minimap { None } else { Some(minimap::render(&built.manifest, &built.tiles).context("rendering the minimap")?) };
    let (manifest, tiles) = built.into_files();
    progress::stage("write");

    let dir = args.out.parent().map(PathBuf::from).unwrap_or_default();
    // Start from an empty tile directory, so tiles of an earlier, different world do not linger.
    let tile_dir = dir.join("tiles");
    if tile_dir.exists() {
        std::fs::remove_dir_all(&tile_dir).with_context(|| format!("clearing {}", tile_dir.display()))?;
    }
    std::fs::create_dir_all(&tile_dir)?;
    let mut tile_bytes = 0;
    for (path, bytes) in &tiles {
        tile_bytes += bytes.len();
        std::fs::write(dir.join(path), bytes).with_context(|| format!("writing {path}"))?;
    }
    if let Some(map) = &map {
        std::fs::write(dir.join("minimap.png"), map).context("writing minimap.png")?;
    }
    std::fs::write(&args.out, &manifest).with_context(|| format!("writing {}", args.out.display()))?;
    eprintln!(
        "wrote {} ({} KiB) and {tile_count} tiles ({} KiB): {buildings} buildings, {roads} roads, {rails} track pieces, {cover} cover patches, {spawns} spawns, {triangles} collision triangles, hash {:08x}",
        args.out.display(),
        manifest.len() / 1024,
        tile_bytes / 1024,
        shared::world::world_hash(&manifest),
    );
    if let Some(line) = cache::summary() {
        eprintln!("{line}");
    }
    Ok(())
}
