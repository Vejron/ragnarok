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

mod assemble;
mod dem;
mod ground;
mod overpass;

use anyhow::{Context, Result, bail};
use clap::Parser;
use std::path::PathBuf;

#[derive(Parser)]
#[command(about = "Generate a Ragnarok world file from OpenStreetMap")]
struct Args {
    /// lat,lon of the world's centre, in degrees. Default: Backen, Umeå.
    #[arg(long, default_value = "63.832127,20.168286")]
    center: String,
    /// Half the side of the square around `--center`, metres.
    #[arg(long, default_value_t = 1000.0)]
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
    /// A Lantmäteriet DTM GeoTIFF (SWEREF 99 TM) to take elevation from. Without it, the DTM is
    /// fetched with the credentials in LM_USER and LM_PASS.
    #[arg(long)]
    dem: Option<PathBuf>,
    /// Skip OpenStreetMap and generate a procedural grid city (works offline).
    #[arg(long)]
    synthetic: bool,
    /// Overpass API endpoints, tried in order (the public servers are often busy).
    #[arg(long, value_delimiter = ',', default_value = "https://overpass-api.de/api/interpreter,https://overpass.private.coffee/api/interpreter")]
    overpass: Vec<String>,
    #[arg(long, default_value = "client/public/world.msgpack")]
    out: PathBuf,
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

/// The terrain under `bbox`'s world, from `--dem` or Lantmäteriet, or flat with `--cell 0`.
fn terrain(args: &Args, bbox: [f64; 4]) -> Result<shared::terrain::Ground> {
    if args.cell == 0.0 {
        return Ok(Default::default());
    }
    let (origin, half) = assemble::frame(bbox);
    // The square of tiles, which may stick out of the playable square, in SWEREF 99 TM.
    let reach = (2.0 * half / args.tile).ceil() * args.tile / 2.0 + args.cell;
    let corners = [(-reach, -reach), (reach, -reach), (-reach, reach), (reach, reach)].map(|(x, z)| {
        let (lat, lon) = assemble::unproject(origin, x, z);
        dem::sweref99tm(lat, lon)
    });
    let fold = |init: f64, pick: fn(f64, f64) -> f64, axis: fn(&(f64, f64)) -> f64| corners.iter().map(axis).fold(init, pick);
    let window = [
        fold(f64::MAX, f64::min, |c| c.0),
        fold(f64::MAX, f64::min, |c| c.1),
        fold(f64::MIN, f64::max, |c| c.0),
        fold(f64::MIN, f64::max, |c| c.1),
    ];
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
    ground::build(half, args.tile, args.cell, |x, z| {
        let (lat, lon) = assemble::unproject(origin, x, z);
        let (e, n) = dem::sweref99tm(lat, lon);
        source.mean(e, n, half_cell).map(f64::from)
    })
}

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.tile >= 20.0, "--tile must be at least 20 m");
    anyhow::ensure!(args.cell == 0.0 || args.cell >= 0.5, "--cell must be 0 or at least 0.5 m");
    let built = if args.synthetic {
        let ground = if args.cell > 0.0 {
            ground::build(300.0, args.tile, args.cell, |x, z| Some(ground::hills(x, z)))?
        } else {
            Default::default()
        };
        assemble::synthetic_city(300.0, args.tile, ground)
    } else {
        let bbox = match &args.bbox {
            Some(text) => parse_bbox(text)?,
            None => {
                let (lat, lon) = parse_center(&args.center)?;
                assemble::bbox_around(lat, lon, args.half)
            }
        };
        // The public servers shed load with 504s; a few rounds over all of them usually get through.
        let mut osm = None;
        'rounds: for round in 0..4 {
            if round > 0 {
                eprintln!("  retrying in 15 s");
                std::thread::sleep(std::time::Duration::from_secs(15));
            }
            for endpoint in &args.overpass {
                eprintln!("querying {endpoint} for {bbox:?}");
                match overpass::fetch(endpoint, bbox) {
                    Ok(result) => {
                        osm = Some(result);
                        break 'rounds;
                    }
                    Err(error) => eprintln!("  failed: {error:#}"),
                }
            }
        }
        let Some(osm) = osm else { bail!("every Overpass endpoint failed; retry later or use --synthetic") };
        let ground = terrain(&args, bbox)?;
        assemble::from_osm(bbox, args.tile, &osm, ground)
    };
    let (buildings, roads, triangles) = built.tiles.values().fold((0, 0, 0), |(b, r, t), tile| (b + tile.buildings.len(), r + tile.roads.len(), t + tile.collision.triangles.len()));
    let cover: usize = built.tiles.values().map(|tile| tile.cover.len()).sum();
    let (spawns, tile_count) = (built.manifest.spawns.len(), built.tiles.len());
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
    std::fs::write(&args.out, &manifest).with_context(|| format!("writing {}", args.out.display()))?;
    eprintln!(
        "wrote {} ({} KiB) and {tile_count} tiles ({} KiB): {buildings} buildings, {roads} roads, {cover} cover patches, {spawns} spawns, {triangles} collision triangles, hash {:08x}",
        args.out.display(),
        manifest.len() / 1024,
        tile_bytes / 1024,
        shared::world::world_hash(&manifest),
    );
    Ok(())
}
