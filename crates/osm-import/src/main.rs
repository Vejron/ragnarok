//! Builds the world from OpenStreetMap: buildings and roads inside a bounding box, projected to
//! local metres, cut into square tiles with their collision meshes, plus the manifest with the
//! spawn points. Writes `world.msgpack` and `tiles/{col}_{row}.msgpack` next to it.
//!
//!     cargo run -p osm-import                                  # Backen, Umeå, 2 km square
//!     cargo run -p osm-import -- --center 59.3167,18.0613 --half 300
//!     cargo run -p osm-import -- --bbox 59.3140,18.0560,59.3195,18.0665
//!     cargo run -p osm-import -- --synthetic                   # offline grid city

mod assemble;
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

fn main() -> Result<()> {
    let args = Args::parse();
    anyhow::ensure!(args.tile >= 20.0, "--tile must be at least 20 m");
    let built = if args.synthetic {
        assemble::synthetic_city(300.0, args.tile)
    } else {
        let bbox = match &args.bbox {
            Some(text) => parse_bbox(text)?,
            None => {
                let (lat, lon) = parse_center(&args.center)?;
                assemble::bbox_around(lat, lon, args.half)
            }
        };
        let mut ways = None;
        for endpoint in &args.overpass {
            eprintln!("querying {endpoint} for {bbox:?}");
            match overpass::fetch(endpoint, bbox) {
                Ok(result) => {
                    ways = Some(result);
                    break;
                }
                Err(error) => eprintln!("  failed: {error:#}"),
            }
        }
        let Some(ways) = ways else { bail!("every Overpass endpoint failed; retry later or use --synthetic") };
        assemble::from_osm(bbox, args.tile, &ways)
    };
    let (buildings, roads, triangles) = built.tiles.values().fold((0, 0, 0), |(b, r, t), tile| (b + tile.buildings.len(), r + tile.roads.len(), t + tile.collision.triangles.len()));
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
        "wrote {} ({} KiB) and {tile_count} tiles ({} KiB): {buildings} buildings, {roads} roads, {spawns} spawns, {triangles} collision triangles, hash {:08x}",
        args.out.display(),
        manifest.len() / 1024,
        tile_bytes / 1024,
        shared::world::world_hash(&manifest),
    );
    Ok(())
}
