mod entities;
mod game;
mod jitter;
mod mechanics;
mod navigation;
mod netem;
mod physics;
mod profile;
mod replication;
mod rng;
mod scenario;
mod session;
mod tls;

use anyhow::{Context, Result};
use certon::{AcmeIssuer, FileStorage, Http01Solver, acme_issuer::CertIssuer};
use salvo::prelude::*;
use shared::world::{Manifest, Tile, WORLD_VERSION, WorldFile, world_hash};
use std::{path::Path, sync::Arc};
use tokio::sync::mpsc;

fn env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// Which `world.msgpack` to load: `RAGNAROK_WORLD`, else the world the map editor activated
/// (`worlds/active.json`), else the generated `client/public` one.
fn world_path() -> String {
    if let Some(path) = env("RAGNAROK_WORLD") {
        return path;
    }
    let active = std::fs::read_to_string("worlds/active.json")
        .ok()
        .and_then(|text| text.split("\"name\":\"").nth(1).and_then(|rest| rest.split('"').next().map(String::from)));
    match active {
        Some(name) if name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_') && !name.is_empty() => {
            let path = format!("worlds/{name}/world.msgpack");
            if Path::new(&path).exists() {
                return path;
            }
            tracing::warn!(%name, "the active world has not been generated; using the default");
            "client/public/world.msgpack".into()
        }
        _ => "client/public/world.msgpack".into(),
    }
}

/// Reads the manifest and every tile next to it, merged into the world the server simulates. The
/// returned hash is the manifest's, which vouches for each tile through the hashes listed in it.
fn load_world() -> Result<(WorldFile, u32)> {
    read_world(Path::new(&world_path()))
}

fn read_world(path: &Path) -> Result<(WorldFile, u32)> {
    let shown = path.display();
    let bytes = std::fs::read(path)
        .with_context(|| format!("reading {shown}; generate it with `cargo run -p osm-import`"))?;
    let manifest = Manifest::decode(&bytes).with_context(|| format!("decoding {shown}"))?;
    anyhow::ensure!(manifest.version == WORLD_VERSION, "{shown} is world version {}, expected {WORLD_VERSION}", manifest.version);
    let dir = path.parent().unwrap_or(Path::new("."));
    let mut tiles = Vec::with_capacity(manifest.tiles.len());
    for entry in &manifest.tiles {
        let file = dir.join(entry.path());
        let tile_bytes = std::fs::read(&file).with_context(|| format!("reading {}", file.display()))?;
        anyhow::ensure!(world_hash(&tile_bytes) == entry.hash, "{} does not match the manifest; regenerate the world", file.display());
        tiles.push((entry.col, entry.row, Tile::decode(&tile_bytes).with_context(|| format!("decoding {}", file.display()))?));
    }
    let mut world = WorldFile::assemble(&manifest, tiles);
    apply_placements(&mut world, &dir.join("area.json"));
    tracing::info!(path = %shown, tiles = manifest.tiles.len(), buildings = world.buildings.len(), roads = world.roads.len(), "world loaded");
    Ok((world, world_hash(&bytes)))
}

/// What the map editor placed by hand, saved in the world's `area.json` as latitude/longitude so
/// it survives the world being re-centred. Missing or unreadable means nothing was placed.
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
/// vehicles and items for the mechanics. Anything outside the world's tiles is dropped.
fn apply_placements(world: &mut WorldFile, path: &Path) {
    let Some(placed) = std::fs::read_to_string(path).ok().and_then(|text| serde_json::from_str::<Placements>(&text).ok()) else { return };
    let origin = world.origin;
    let locate = |world: &WorldFile, lat: f64, lon: f64| {
        let [x, z] = shared::world::project(origin, lat, lon);
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
}

/// `RAGNAROK_BOTS=n` adds `n` bots on top of any placed ones, for trying them out: each on the bot
/// team at one of the world's spawn points just past the few players start at, on a made-up loop.
fn add_test_bots(game: &mut game::Game) {
    let Some(count) = env("RAGNAROK_BOTS").and_then(|n| n.parse::<usize>().ok()) else { return };
    let spawns = game.world.map.spawns.clone();
    for n in 0..count {
        let at = spawns.get((n + 6) % spawns.len().max(1)).copied().unwrap_or([0, 0]);
        if game.add_bot(at.map(shared::quant::from_cm), game::TEAM_BOTS, Vec::new()).is_none() {
            tracing::warn!(added = n, "MAX_BOTS reached, no more test bots");
            break;
        }
    }
}

/// `RAGNAROK_TANKS=n` adds `n` tank crews on the bot team, for trying them out: each tank at one of
/// the world's spawn points, from the last back, where one fits, driving along the nearest road.
fn add_test_tanks(game: &mut game::Game) {
    let Some(count) = env("RAGNAROK_TANKS").and_then(|n| n.parse::<usize>().ok()) else { return };
    let spawns = game.world.map.spawns.clone();
    let mut added = 0;
    for at in spawns.iter().rev() {
        if added == count {
            break;
        }
        let names = [format!("Tank {} driver", added + 1), format!("Tank {} gunner", added + 1)];
        if game.add_crew(names, at.map(shared::quant::from_cm), game::TEAM_BOTS, Vec::new()).is_some() {
            added += 1;
        }
    }
    if added < count {
        tracing::warn!(added, wanted = count, "room for fewer test tanks than asked for");
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let (world, hash) = load_world()?;
    let (commands, receiver) = mpsc::channel(session::COMMAND_CAPACITY);
    let mut game = game::Game::new(world, hash)?;
    let scenario_path = Path::new(&world_path()).with_file_name("scenario.json");
    game.scenario = scenario::load(&scenario_path, &game.world.map)?;
    add_test_bots(&mut game);
    add_test_tanks(&mut game);
    let game_task = tokio::spawn(game::run(game, receiver));
    // The sessions would keep accepting players into a dead simulation, so end the process and let
    // the supervisor restart it.
    tokio::spawn(async move {
        let outcome = game_task.await;
        tracing::error!(?outcome, "game loop ended, exiting");
        std::process::exit(1);
    });

    let netem = netem::NetemConfig::from_env();
    if netem.enabled() {
        tracing::warn!(?netem, "simulating a bad network");
    }
    let domain = env("RAGNAROK_DOMAIN");
    let mut router = Router::new().push(Router::with_path("transport").goal(session::Transport {
        commands,
        domain: domain.clone(),
        netem,
    }));

    if let Some(domain) = domain {
        let dist = env("RAGNAROK_DIST").unwrap_or_else(|| "client/dist".into());
        router = router.push(Router::with_path("{*path}").get(StaticDir::new([dist]).defaults("index.html")));
        let cache = env("RAGNAROK_ACME_CACHE").unwrap_or_else(|| "crates/server/acme".into());
        let staging = env("RAGNAROK_ACME_STAGING").is_some();
        let mut issuer = AcmeIssuer::builder()
            .agreed(true)
            .storage(Arc::new(FileStorage::new(&cache)))
            .http01_solver(Arc::new(Http01Solver::new(80)));
        if staging {
            issuer = issuer.ca("https://acme-staging-v02.api.letsencrypt.org/directory");
        }
        let issuer = issuer.build();
        let cert_path = Path::new(&cache)
            .join("certificates")
            .join(issuer.issuer_key())
            .join(&domain)
            .join(format!("{domain}.crt"));
        let had_certificate = cert_path.exists();
        let mut listener = TcpListener::new("0.0.0.0:443")
            .acme()
            .cache_path(cache)
            .add_domain(domain)
            .add_issuer(Arc::new(issuer));
        if staging {
            listener = listener.directory("staging", "https://acme-staging-v02.api.letsencrypt.org/directory");
        }
        let listener = listener
            .http01_challenge(&mut router)
            .quinn("0.0.0.0:443")
            .join(TcpListener::new("0.0.0.0:80"))
            .bind()
            .await;
        if !had_certificate && cert_path.exists() {
            anyhow::bail!("Initial ACME certificate issued; restart to load it into the TLS cache");
        }
        Server::new(listener).serve(router).await;
    } else {
        let (config, hash) = tls::local_certificate()?;
        router = router.push(Router::with_path("certificate-hash").get(hash));
        tracing::info!("dev server: WebTransport on https://127.0.0.1:8698/transport, cert hash on :8699");
        let listener = QuinnListener::new(config.clone(), ("127.0.0.1", 8698))
            .join(TcpListener::new(("127.0.0.1", 8698)).rustls(config))
            .join(TcpListener::new(("127.0.0.1", 8699)))
            .bind()
            .await;
        Server::new(listener).serve(router).await;
    }
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
