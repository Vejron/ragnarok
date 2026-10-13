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
//!
//! Credentials, data sources and the cache are described in `WORLD.md`.

mod assemble;
mod bridge;
mod cache;
mod canopy;
mod dem;
mod detail;
mod ground;
mod hydro;
mod lidar;
mod minimap;
mod overpass;
mod props;
mod roof;
mod trees;
mod water;

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
    /// Half the side of the square around `--center`, metres. The default is as large as the
    /// terrain allows here: a bigger square takes in a hill that breaks the 100 m relief limit.
    #[arg(long, default_value_t = 4300.0)]
    half: f64,
    /// south,west,north,east in degrees. Overrides `--center` and `--half`.
    #[arg(long)]
    bbox: Option<String>,
    /// Side of a tile, metres.
    #[arg(long, default_value_t = 250.0)]
    tile: f64,
    /// Distance between terrain height samples, metres; must divide `--tile`. 0 makes the ground flat.
    #[arg(long, default_value_t = 5.0)]
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
    /// Leave out lakes, rivers and streams (Lantmäteriet's Hydrografi Direkt).
    #[arg(long)]
    no_water: bool,
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

/// The SWEREF 99 TM window `[e0, n0, e1, n1]` over the world's whole grid of tiles.
fn sweref_window(args: &Args, bbox: [f64; 4]) -> dem::Window {
    let (origin, half) = assemble::frame(bbox);
    // The square of tiles, which may stick out of the playable square, in SWEREF 99 TM.
    let reach = (2.0 * half / args.tile).ceil() * args.tile / 2.0 + args.cell;
    let corners = [(-reach, -reach), (reach, -reach), (-reach, reach), (reach, reach)].map(|(x, z)| {
        let (lat, lon) = assemble::unproject(origin, x, z);
        dem::sweref99tm(lat, lon)
    });
    let fold = |init: f64, pick: fn(f64, f64) -> f64, axis: fn(&(f64, f64)) -> f64| corners.iter().map(axis).fold(init, pick);
    [
        fold(f64::MAX, f64::min, |c| c.0),
        fold(f64::MAX, f64::min, |c| c.1),
        fold(f64::MIN, f64::max, |c| c.0),
        fold(f64::MIN, f64::max, |c| c.1),
    ]
}

/// Lakes, rivers and streams from Lantmäteriet's Hydrografi, in local metres. Empty with
/// `--no-water`, without credentials or on flat ground (the surface height comes from the DTM).
fn water_map(args: &Args, bbox: [f64; 4]) -> Result<water::Map> {
    if args.no_water || args.cell == 0.0 {
        return Ok(water::Map::default());
    }
    let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
        eprintln!("no LM_USER / LM_PASS: the world has no water");
        return Ok(water::Map::default());
    };
    let (origin, half) = assemble::frame(bbox);
    // The whole grid of tiles, which may stick out of the playable square.
    let reach = (2.0 * half / args.tile).ceil() * args.tile / 2.0 + args.cell;
    let (south, west) = assemble::unproject(origin, -reach, reach);
    let (north, east) = assemble::unproject(origin, reach, -reach);
    let found = hydro::fetch([south, west, north, east], &user, &password).context("Hydrografi failed; retry later or use --no-water")?;
    let local = |ring: &[overpass::LatLon]| -> Vec<[f64; 2]> { ring.iter().map(|p| assemble::project(origin, p.lat, p.lon)).collect() };
    // Rings are closed in GeoJSON: drop the repeated first point.
    let open = |mut ring: Vec<[f64; 2]>| {
        if ring.len() > 1 && ring.first() == ring.last() {
            ring.pop();
        }
        ring
    };
    let mut map = water::Map::default();
    for (polygons, kind) in [(&found.lakes, water::Kind::Lake), (&found.rivers, water::Kind::River)] {
        for polygon in polygons {
            map.bodies.push(water::Body { kind, rings: polygon.iter().map(|ring| open(local(ring))).filter(|ring| ring.len() >= 3).collect() });
        }
    }
    map.streams = found.streams.iter().map(|line| local(line)).collect();
    eprintln!("water: {} lakes, {} river polygons, {} stream lines from Hydrografi", found.lakes.len(), found.rivers.len(), found.streams.len());
    Ok(map)
}

/// The terrain under `bbox`'s world, the elevation of its y = 0 and the water over it, from
/// `--dem` or Lantmäteriet, or flat with `--cell 0`. Lakes and rivers are carved into the ground.
fn terrain(args: &Args, bbox: [f64; 4], map: &water::Map) -> Result<(shared::terrain::Ground, f64, Option<dem::Dem>, water::Placed)> {
    if args.cell == 0.0 {
        return Ok((Default::default(), 0.0, None, Default::default()));
    }
    let (origin, half) = assemble::frame(bbox);
    let window = sweref_window(args, bbox);
    let source = match &args.dem {
        Some(path) => dem::Dem::from_path(path, window)?,
        None => {
            let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
                bail!("elevation needs --dem <file.tif>, or LM_USER and LM_PASS (Lantmäteriet credentials), or --cell 0 for flat ground");
            };
            dem::Dem::from_stac(bbox, window, &user, &password)?
        }
    };
    let half_cell = args.cell / 2.0;
    let dtm = |x: f64, z: f64| {
        let (lat, lon) = assemble::unproject(origin, x, z);
        let (e, n) = dem::sweref99tm(lat, lon);
        source.mean(e, n, half_cell).map(f64::from)
    };
    let surface = water::Surface::new(map, ground::grid(half, args.tile, args.cell)?, dtm);
    let (ground, datum) = ground::build(half, args.tile, args.cell, dtm, |x, z, dem| surface.bed(x, z, dem))?;
    let placed = water::Placed::new(map, &surface, datum, &ground);
    report(map, &placed);
    Ok((ground, datum, Some(source), placed))
}

fn report(map: &water::Map, placed: &water::Placed) {
    let wet = placed.levels.iter().filter(|&&level| level != shared::world::NO_WATER).count();
    eprintln!("water: {} bodies, {} samples under or beside water, {} stream lines kept", map.bodies.len(), wet, placed.streams.len());
}

/// What the laser data gives: the returns that can be roof, and the canopy for trees. Each is
/// `None` when switched off or without credentials (`--no-roofs`, `--no-lidar-trees`).
fn laser(args: &Args, bbox: [f64; 4], datum: f64, osm: &overpass::Osm, dem: Option<&dem::Dem>) -> Result<(Option<roof::Cloud>, Vec<canopy::Detected>)> {
    let want_roofs = !args.no_roofs;
    let want_trees = !args.no_lidar_trees && dem.is_some();
    if args.cell == 0.0 || !(want_roofs || want_trees) {
        return Ok((None, Vec::new()));
    }
    let (Some(user), Some(password)) = (std::env::var("LM_USER").ok(), std::env::var("LM_PASS").ok()) else {
        eprintln!("no LM_USER / LM_PASS: buildings stay boxes and only the register's trees are planted");
        return Ok((None, Vec::new()));
    };
    let window = sweref_window(args, bbox);
    let mut cloud = want_roofs.then(|| roof::Cloud::new(window));
    let mut canopy = want_trees.then(|| canopy::Canopy::new(window));
    let total = lidar::fetch(bbox, window, &user, &password, |p| {
        if let Some(cloud) = &mut cloud {
            cloud.add(&p, datum);
        }
        if let Some(canopy) = &mut canopy {
            canopy.add(&p);
        }
    })?;
    eprintln!("laser: {total} returns{}", cloud.as_ref().map_or(String::new(), |c| format!(", {} kept for roofs", c.len())));
    let mut detected = Vec::new();
    if let (Some(mut canopy), Some(dem)) = (canopy, dem) {
        // Roofs are not trees: keep the canopy off every building outline.
        for way in osm.ways.iter().filter(|way| way.tags.contains_key("building")) {
            let ring: Vec<(f64, f64)> = way.geometry.iter().map(|p| dem::sweref99tm(p.lat, p.lon)).collect();
            canopy.mask(&ring, 1.5);
        }
        detected = canopy.detect(|e, n| dem.mean(e, n, 0.5), args.min_tree_height);
        eprintln!("canopy: {} trees at least {} m tall", detected.len(), args.min_tree_height);
    }
    Ok((cloud, detected))
}

fn main() -> Result<()> {
    let args = Args::parse();
    if args.clear_cache {
        let freed = cache::clear(&args.cache)?;
        eprintln!("cleared {} ({} MiB)", args.cache.display(), freed / (1024 * 1024));
        return Ok(());
    }
    cache::init((!args.no_cache).then(|| args.cache.clone()), args.refresh);
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
        let bbox = match &args.bbox {
            Some(text) => parse_bbox(text)?,
            None => {
                let (lat, lon) = parse_center(&args.center)?;
                assemble::bbox_around(lat, lon, args.half)
            }
        };
        // Before the network-flaky step: a world that cannot fit the format should fail in seconds.
        let map = water_map(&args, bbox)?;
        let (ground, datum, dem, placed) = terrain(&args, bbox, &map)?;
        // The public servers shed load with 504s; a few rounds over all of them usually get through.
        let osm = overpass::fetch_chunked(&args.overpass, bbox, args.overpass_chunk, 3, 15).context("Overpass failed; retry later or use --synthetic")?;
        let (cloud, detected) = laser(&args, bbox, datum, &osm, dem.as_ref())?;
        let register = if args.no_trees {
            Vec::new()
        } else {
            match trees::fetch(bbox) {
                Ok(planted) => {
                    eprintln!("trees: {} from Umeå open data", planted.len());
                    planted
                }
                // The tree data is a nicety; a world without it is still a world.
                Err(error) => {
                    eprintln!("no trees: {error:#}");
                    Vec::new()
                }
            }
        };
        let planted = trees::merge(register, &detected);
        eprintln!("trees: {} from the register, {} from the laser", planted.iter().filter(|t| t.registered).count(), planted.iter().filter(|t| !t.registered).count());
        let coarse = (args.detail > 0.0).then(|| ground.clone());
        let mut built = assemble::from_osm(bbox, args.tile, &osm, &planted, ground, placed, cloud.as_ref());
        if let (Some(coarse), Some(source)) = (coarse, dem.as_ref()) {
            let (origin, _) = assemble::frame(bbox);
            let model = |half: f64| {
                move |x: f64, z: f64| {
                    let (lat, lon) = assemble::unproject(origin, x, z);
                    let (e, n) = dem::sweref99tm(lat, lon);
                    source.mean(e, n, half).map(f64::from)
                }
            };
            detail::bake(&mut built, &coarse, datum, args.detail, model(args.cell / 2.0), model(args.detail / 2.0));
        }
        built
    };
    let (buildings, roads, triangles) = built.tiles.values().fold((0, 0, 0), |(b, r, t), tile| (b + tile.buildings.len(), r + tile.roads.len(), t + tile.collision.triangles.len()));
    let cover: usize = built.tiles.values().map(|tile| tile.cover.len()).sum();
    let mut by_kind = [0usize; 7];
    for tree in built.tiles.values().flat_map(|tile| &tile.trees) {
        by_kind[usize::from(tree.kind).min(6)] += 1;
    }
    eprintln!(
        "trees: {} birch, {} pine, {} spruce, {} ornamental, {} broadleaf, {} columnar, {} low-wide",
        by_kind[0], by_kind[1], by_kind[2], by_kind[3], by_kind[4], by_kind[5], by_kind[6]
    );
    let mut roofs = [0usize; 4];
    for building in built.tiles.values().flat_map(|tile| &tile.buildings) {
        roofs[usize::from(building.roof_kind).min(3)] += 1;
    }
    eprintln!("roofs: {} flat, {} shed, {} gable, {} hip", roofs[0], roofs[1], roofs[2], roofs[3]);
    let (spawns, tile_count) = (built.manifest.spawns.len(), built.tiles.len());
    let map = if args.no_minimap { None } else { Some(minimap::render(&built.manifest, &built.tiles).context("rendering the minimap")?) };
    let (manifest, tiles) = built.into_files();

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
        "wrote {} ({} KiB) and {tile_count} tiles ({} KiB): {buildings} buildings, {roads} roads, {cover} cover patches, {spawns} spawns, {triangles} collision triangles, hash {:08x}",
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
