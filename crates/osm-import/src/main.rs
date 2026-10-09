//! Builds `world.msgpack` from OpenStreetMap: buildings and roads inside a bounding box,
//! projected to local metres, plus the static collision mesh and spawn points.
//!
//!     cargo run -p osm-import                                  # default block
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
    /// south,west,north,east in degrees. Default: a gridded block of Södermalm, Stockholm (~600 m).
    #[arg(long, default_value = "59.3140,18.0560,59.3195,18.0665")]
    bbox: String,
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

fn main() -> Result<()> {
    let args = Args::parse();
    let world = if args.synthetic {
        assemble::synthetic_city(300.0)
    } else {
        let bbox = parse_bbox(&args.bbox)?;
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
        assemble::from_osm(bbox, &ways)
    };
    let bytes = world.encode();
    if let Some(parent) = args.out.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&args.out, &bytes).with_context(|| format!("writing {}", args.out.display()))?;
    eprintln!(
        "wrote {} ({} KiB): {} buildings, {} roads, {} spawns, {} collision triangles, hash {:08x}",
        args.out.display(),
        bytes.len() / 1024,
        world.buildings.len(),
        world.roads.len(),
        world.spawns.len(),
        world.collision.triangles.len(),
        shared::world::world_hash(&bytes),
    );
    Ok(())
}
