mod game;
mod jitter;
mod mechanics;
mod netem;
mod physics;
mod replication;
mod rng;
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

/// Reads the manifest and every tile next to it, merged into the world the server simulates. The
/// returned hash is the manifest's, which vouches for each tile through the hashes listed in it.
fn load_world() -> Result<(WorldFile, u32)> {
    let path = env("RAGNAROK_WORLD").unwrap_or_else(|| "client/public/world.msgpack".into());
    read_world(Path::new(&path))
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
    let world = WorldFile::assemble(&manifest, tiles);
    tracing::info!(path = %shown, tiles = manifest.tiles.len(), buildings = world.buildings.len(), roads = world.roads.len(), "world loaded");
    Ok((world, world_hash(&bytes)))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let (world, hash) = load_world()?;
    let (commands, receiver) = mpsc::channel(session::COMMAND_CAPACITY);
    let game_task = tokio::spawn(game::run(game::Game::new(world, hash)?, receiver));
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
