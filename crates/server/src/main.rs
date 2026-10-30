mod entities;
mod events;
mod game;
mod jitter;
mod mechanics;
mod navigation;
mod netem;
mod placements;
mod profile;
mod replication;
mod rng;
mod scenario;
mod session;
mod tls;

use anyhow::{Context, Result};
use certon::{AcmeIssuer, FileStorage, Http01Solver, acme_issuer::CertIssuer};
use salvo::prelude::*;
use shared::world::{Manifest, Tile, WORLD_VERSION, WorldFile, plain_world_name, world_hash};
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
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        .and_then(|value| value["name"].as_str().map(String::from))
        .filter(|name| plain_world_name(name));
    if let Some(name) = active {
        let path = format!("worlds/{name}/world.msgpack");
        if Path::new(&path).exists() {
            return path;
        }
        tracing::warn!(%name, "the active world has not been generated; using the default");
    }
    "client/public/world.msgpack".into()
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
    placements::apply(&mut world, &dir.join("area.json"))?;
    tracing::info!(path = %shown, tiles = manifest.tiles.len(), buildings = world.buildings.len(), roads = world.roads.len(), "world loaded");
    Ok((world, world_hash(&bytes)))
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
    if let Some(home) = game.scenario.as_ref().and_then(scenario::Scenario::backpack) {
        tracing::info!(?home, "backpack mode");
        game.world.backpack = Some(mechanics::BackpackState::new(home));
    }
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
