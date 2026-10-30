//! What the map editor placed by hand in a world: its spawn points, vehicles, items and bots.

use anyhow::{Context, Result};
use shared::world::WorldFile;
use std::path::Path;

/// What the map editor placed by hand, saved in the world's `area.json` as latitude/longitude so
/// it survives the world being re-centred.
#[derive(serde::Deserialize, Default)]
struct Placements {
    #[serde(default)]
    spawns: Vec<Spot>,
    #[serde(default)]
    vehicles: Vec<PlacedVehicleSpot>,
    #[serde(default)]
    items: Vec<PlacedItemSpot>,
    #[serde(default)]
    bots: Vec<PlacedBotSpot>,
}

#[derive(serde::Deserialize)]
struct Spot {
    lat: f64,
    lon: f64,
}

#[derive(serde::Deserialize)]
struct PlacedVehicleSpot {
    kind: u8,
    lat: f64,
    lon: f64,
    #[serde(default)]
    yaw: f32,
}

#[derive(serde::Deserialize)]
struct PlacedItemSpot {
    item: u8,
    #[serde(default = "one")]
    count: u8,
    lat: f64,
    lon: f64,
}

#[derive(serde::Deserialize)]
struct PlacedBotSpot {
    lat: f64,
    lon: f64,
    #[serde(default = "one")]
    team: u8,
    /// How good a shot it is, 0 to 1.
    #[serde(default = "half")]
    skill: f32,
    /// Waypoints it walks round; empty makes up a short loop around where it stands.
    #[serde(default)]
    route: Vec<Spot>,
}

fn one() -> u8 {
    1
}

fn half() -> f32 {
    0.5
}

/// Replaces the world's spawn points with the editor's, if it placed any, and records the placed
/// vehicles and items for the mechanics. Anything outside the world's tiles is dropped. No file means
/// nothing was placed; one that does not read is an error, as a scenario that does not is, rather than
/// a world quietly missing its bots.
pub fn apply(world: &mut WorldFile, path: &Path) -> Result<()> {
    let Ok(text) = std::fs::read_to_string(path) else { return Ok(()) };
    let placed: Placements = serde_json::from_str(&text).with_context(|| format!("reading {}", path.display()))?;
    let frame = world.frame;
    let locate = |world: &WorldFile, lat: f64, lon: f64| {
        let [x, z] = frame.project(lat, lon);
        let at = [(x * 100.0).round() as i32, (z * 100.0).round() as i32];
        world.is_inside(at[0], at[1]).then_some(at)
    };
    let spawns: Vec<[i32; 2]> = placed.spawns.iter().filter_map(|s| locate(world, s.lat, s.lon)).collect();
    if !spawns.is_empty() {
        world.spawns = spawns;
    }
    world.placed_vehicles = placed
        .vehicles
        .iter()
        .filter_map(|v| locate(world, v.lat, v.lon).map(|at| shared::world::PlacedVehicle { kind: v.kind, at, yaw: v.yaw }))
        .collect();
    world.placed_items = placed
        .items
        .iter()
        .filter_map(|i| locate(world, i.lat, i.lon).map(|at| shared::world::PlacedItem { item: i.item, count: i.count.max(1), at }))
        .collect();
    // A waypoint outside the world is dropped; the bot walks the rest.
    world.placed_bots = placed
        .bots
        .iter()
        .filter_map(|b| {
            let route = b.route.iter().filter_map(|p| locate(world, p.lat, p.lon)).collect();
            locate(world, b.lat, b.lon).map(|at| shared::world::PlacedBot { at, team: b.team, skill: b.skill, route })
        })
        .collect();
    tracing::info!(spawns = world.spawns.len(), vehicles = world.placed_vehicles.len(), items = world.placed_items.len(), bots = world.placed_bots.len(), "editor placements applied");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn placed_bots_read_as_the_editor_writes_them_with_defaults_for_what_is_left_out() {
        let text = r#"{"bots":[{"lat":63.82,"lon":20.29,"team":2,"skill":0.8,"route":[{"lat":63.83,"lon":20.3}]},{"lat":63.8,"lon":20.2}]}"#;
        let placed: Placements = serde_json::from_str(text).unwrap();
        let [a, b] = &placed.bots[..] else { panic!("two bots") };
        assert_eq!((a.team, a.skill, a.route.len()), (2, 0.8, 1));
        assert_eq!((b.team, b.skill, b.route.len()), (1, 0.5, 0));
    }
}
