//! What is in an OCAD file: its version, how it sits on the ground and how many objects it has of
//! each symbol. For checking a new map before building a world from it.

use anyhow::{Context, Result, bail};
use osm_import::ocad::file::{self, OBJECT_AREA, OBJECT_LINE, OBJECT_POINT};
use std::collections::BTreeMap;

fn main() -> Result<()> {
    let Some(path) = std::env::args().nth(1) else { bail!("usage: ocd-dump <map.ocd>") };
    let bytes = std::fs::read(&path).with_context(|| format!("reading {path}"))?;
    let map = file::parse(&bytes).with_context(|| format!("parsing {path}"))?;
    println!("version: {}.{}", map.version, map.subversion);
    let file::Scale { scale, x, y, angle, grid, real } = map.scale;
    println!("scale: 1:{scale}");
    println!("georeference: origin ({x}, {y}), rotation {angle}°, grid code {grid}, real-world coordinates {}", if real { "on" } else { "off" });
    println!("symbols: {}, objects: {}, encrypted: {}", map.symbols.len(), map.objects.len(), map.encrypted);
    // Paper bounds, 1/100 mm.
    let points = || map.objects.iter().flat_map(|object| &object.rings).flatten();
    let (lo, hi) = points().fold(([f64::MAX; 2], [f64::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    if lo[0] <= hi[0] {
        let metres = scale / 100_000.0;
        println!("extent on the ground: {:.0} × {:.0} m", (hi[0] - lo[0]) * metres, (hi[1] - lo[1]) * metres);
        println!("paper bounds: ({}, {})..({}, {})", lo[0], lo[1], hi[0], hi[1]);
    }
    let mut count: BTreeMap<i32, [usize; 3]> = BTreeMap::new();
    for object in &map.objects {
        let slot = match object.kind {
            OBJECT_POINT => 0,
            OBJECT_LINE => 1,
            OBJECT_AREA => 2,
            _ => continue,
        };
        count.entry(object.symbol).or_default()[slot] += 1;
    }
    println!("{:>8} {:>7} {:>7} {:>7}  description", "symbol", "points", "lines", "areas");
    for (symbol, [points, lines, areas]) in count {
        let description = map.symbols.get(&symbol).map_or("?", |symbol| symbol.description.as_str());
        println!("{:>8.3} {points:>7} {lines:>7} {areas:>7}  {description}", f64::from(symbol) / 1000.0);
    }
    Ok(())
}
