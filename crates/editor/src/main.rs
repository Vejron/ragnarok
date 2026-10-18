//! The map editor's local API: saved worlds (a set of 250 m tiles on the global grid), the state
//! of the importer's download cache over a map window, and a job that downloads what a world needs.
//!
//!     cargo run -p editor            # http://127.0.0.1:8700, with the client's Vite proxying /api
//!
//! Worlds live in `worlds/<name>/area.json` (`RAGNAROK_WORLDS` to move them). The cache is the
//! importer's own (`.cache/osm-import`, or `RAGNAROK_CACHE`), so what the editor downloads is what
//! `osm-import` reuses. Credentials come from `.env.local`, as for the importer.

use osm_import::grid::{self, BLOCK_TILES, Block, Tile};
use osm_import::{cache, hydro, overpass, progress, trees};
use salvo::prelude::*;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Instant;

/// The most blocks one cache query answers: a window of about 40 km.
const MAX_BLOCKS: i64 = 400;

fn worlds_dir() -> PathBuf {
    std::env::var("RAGNAROK_WORLDS").map_or_else(|_| PathBuf::from("worlds"), PathBuf::from)
}

/// Worlds are directories named by the player, so keep the names plain.
fn valid(name: &str) -> bool {
    (1..=40).contains(&name.len()) && name.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-' || b == b'_')
}

#[derive(Deserialize, Serialize)]
struct Area {
    tiles: Vec<Tile>,
    /// Hand-placed spawn points, vehicles, items and bots (latitude/longitude). The server reads
    /// these from the same file; the importer ignores them.
    #[serde(default)]
    spawns: Vec<serde_json::Value>,
    #[serde(default)]
    vehicles: Vec<serde_json::Value>,
    #[serde(default)]
    items: Vec<serde_json::Value>,
    #[serde(default)]
    bots: Vec<serde_json::Value>,
}

fn read_area(name: &str) -> Option<Area> {
    let text = std::fs::read_to_string(worlds_dir().join(name).join("area.json")).ok()?;
    serde_json::from_str(&text).ok()
}

fn fail(res: &mut Response, code: StatusCode, message: &str) {
    res.status_code(code);
    res.render(Json(serde_json::json!({ "error": message })));
}

fn name_of(req: &Request, res: &mut Response) -> Option<String> {
    let name: String = req.param("name").unwrap_or_default();
    if valid(&name) {
        Some(name)
    } else {
        fail(res, StatusCode::BAD_REQUEST, "a world name is 1-40 characters of a-z, 0-9, - and _");
        None
    }
}

#[handler]
async fn grid_info(res: &mut Response) {
    res.render(Json(serde_json::json!({
        "origin": [grid::ORIGIN.0, grid::ORIGIN.1],
        "tile": grid::TILE,
        "blockTiles": BLOCK_TILES,
        "metresPerDegLat": 110_540.0,
        "metresPerDegLon": 111_320.0,
    })));
}

#[handler]
async fn list_worlds(res: &mut Response) {
    let active = std::fs::read_to_string(worlds_dir().join("active.json"))
        .ok()
        .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
        .and_then(|v| v["name"].as_str().map(String::from));
    let mut worlds = Vec::new();
    for entry in std::fs::read_dir(worlds_dir()).into_iter().flatten().flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if let (true, Some(area)) = (valid(&name), read_area(&name)) {
            let generated = worlds_dir().join(&name).join("world.msgpack").exists();
            worlds.push(serde_json::json!({ "name": name, "tiles": area.tiles.len(), "active": active.as_deref() == Some(&name), "generated": generated }));
        }
    }
    worlds.sort_by_key(|w| w["name"].as_str().unwrap_or_default().to_string());
    res.render(Json(serde_json::json!({ "worlds": worlds })));
}

#[handler]
async fn get_world(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    match read_area(&name) {
        Some(area) => res.render(Json(serde_json::json!({ "name": name, "tiles": area.tiles, "spawns": area.spawns, "vehicles": area.vehicles, "items": area.items, "bots": area.bots }))),
        None => fail(res, StatusCode::NOT_FOUND, "no such world"),
    }
}

#[handler]
async fn put_world(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    let Ok(mut area) = req.parse_json::<Area>().await else { return fail(res, StatusCode::BAD_REQUEST, "expected {\"tiles\": [{\"gx\":0,\"gy\":0}]}") };
    area.tiles.sort();
    area.tiles.dedup();
    let dir = worlds_dir().join(&name);
    let written = std::fs::create_dir_all(&dir).and_then(|()| std::fs::write(dir.join("area.json"), serde_json::to_vec(&area).unwrap_or_default()));
    match written {
        Ok(()) => res.render(Json(serde_json::json!({ "name": name, "tiles": area.tiles.len() }))),
        Err(error) => fail(res, StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

#[handler]
async fn delete_world(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    match std::fs::remove_dir_all(worlds_dir().join(&name)) {
        Ok(()) => res.render(Json(serde_json::json!({ "deleted": name }))),
        Err(error) => fail(res, StatusCode::NOT_FOUND, &error.to_string()),
    }
}

#[handler]
async fn activate(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    if read_area(&name).is_none() {
        return fail(res, StatusCode::NOT_FOUND, "no such world");
    }
    let text = serde_json::json!({ "name": name }).to_string();
    match std::fs::write(worlds_dir().join("active.json"), text) {
        Ok(()) => res.render(Json(serde_json::json!({ "active": name }))),
        Err(error) => fail(res, StatusCode::INTERNAL_SERVER_ERROR, &error.to_string()),
    }
}

/// The cache state of every block that touches a window, `south`, `west`, `north`, `east` in degrees.
#[handler]
async fn cache_state(req: &mut Request, res: &mut Response) {
    let q = |key: &str| req.query::<f64>(key);
    let (Some(south), Some(west), Some(north), Some(east)) = (q("south"), q("west"), q("north"), q("east")) else {
        return fail(res, StatusCode::BAD_REQUEST, "south, west, north and east are required");
    };
    // Block (0, 0) is not at the grid's corner tile of the window, so take all four corners.
    let corners = [Tile::at(south, west).block(), Tile::at(south, east).block(), Tile::at(north, west).block(), Tile::at(north, east).block()];
    let (bx0, bx1) = (corners.iter().map(|b| b.bx).min().unwrap_or(0), corners.iter().map(|b| b.bx).max().unwrap_or(0));
    let (by0, by1) = (corners.iter().map(|b| b.by).min().unwrap_or(0), corners.iter().map(|b| b.by).max().unwrap_or(0));
    if i64::from(bx1 - bx0 + 1) * i64::from(by1 - by0 + 1) > MAX_BLOCKS {
        return fail(res, StatusCode::BAD_REQUEST, "window too large; zoom in");
    }
    let mut blocks = Vec::new();
    for by in by0..=by1 {
        for bx in bx0..=bx1 {
            let s = Block { bx, by }.status();
            blocks.push(serde_json::json!({ "bx": bx, "by": by, "overpass": s.overpass, "hydro": s.hydro, "trees": s.trees }));
        }
    }
    res.render(Json(serde_json::json!({ "blocks": blocks })));
}

#[derive(Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
struct Job {
    world: String,
    /// `download` (fill the cache) or `generate` (build the world files).
    kind: &'static str,
    /// `idle`, `running`, `done` or `failed`.
    state: &'static str,
    /// Blocks downloaded (a download), or 0 and 1 (a generate).
    done: usize,
    total: usize,
    /// The stage now running, its steps, and the bytes and rate so far.
    progress: progress::Snapshot,
    /// Milliseconds from the start to now, or to the end once the job has ended.
    elapsed_ms: u64,
    #[serde(skip)]
    started: Option<Instant>,
    #[serde(skip)]
    ended: Option<Instant>,
    /// What went wrong, or notes (a block without water, for example).
    messages: Vec<String>,
}

static JOB: Mutex<Job> = Mutex::new(Job {
    world: String::new(),
    kind: "download",
    state: "idle",
    done: 0,
    total: 0,
    progress: progress::Snapshot { stage: String::new(), step: 0, steps: 0, bytes: 0, bytes_per_second: 0.0, cached: 0, cached_per_second: 0.0 },
    elapsed_ms: 0,
    started: None,
    ended: None,
    messages: Vec::new(),
});

fn job() -> std::sync::MutexGuard<'static, Job> {
    JOB.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Downloads, block by block, what the world's tiles need, into the importer's cache.
fn run(blocks: Vec<Block>, credentials: Option<(String, String)>) {
    let endpoints: Vec<String> = overpass::DEFAULT_ENDPOINTS.iter().map(|e| e.to_string()).collect();
    for block in blocks {
        let bbox = block.bbox();
        progress::stage("overpass");
        if let Err(error) = overpass::fetch_pieces(&endpoints, &[bbox], 3, 15) {
            let mut job = job();
            job.state = "failed";
            job.ended = Some(Instant::now());
            job.messages.push(format!("block ({}, {}): {error:#}", block.bx, block.by));
            return;
        }
        // Water and trees are extras: note what is missing and carry on.
        let mut notes = Vec::new();
        if let Some((user, password)) = &credentials {
            progress::stage("water");
            if let Err(error) = hydro::fetch_pieces(&[bbox], user, password) {
                notes.push(format!("block ({}, {}) has no water: {error:#}", block.bx, block.by));
            }
        }
        progress::stage("trees");
        if let Err(error) = trees::fetch(bbox) {
            notes.push(format!("block ({}, {}) has no trees: {error:#}", block.bx, block.by));
        }
        let mut job = job();
        job.done += 1;
        job.messages.extend(notes);
    }
    let mut job = job();
    job.state = "done";
    job.ended = Some(Instant::now());
}

#[handler]
async fn start_fetch(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    let Some(area) = read_area(&name) else { return fail(res, StatusCode::NOT_FOUND, "no such world") };
    let blocks: Vec<Block> = grid::blocks_of(&area.tiles).into_iter().filter(|b| !b.status().ready()).collect();
    {
        let mut current = job();
        if current.state == "running" {
            return fail(res, StatusCode::CONFLICT, "a download is already running");
        }
        *current = Job { world: name, kind: "download", state: "running", total: blocks.len(), started: Some(Instant::now()), ..Job::default() };
        if blocks.is_empty() {
            current.state = "done";
            current.ended = Some(Instant::now());
        }
    }
    progress::reset();
    if !blocks.is_empty() {
        let credentials = credential("LM_USER").zip(credential("LM_PASS"));
        std::thread::spawn(move || run(blocks, credentials));
    }
    res.render(Json(current()));
}

/// Builds the world's files by running the importer on its `area.json`: the same program and the
/// same cache as `cargo run -p osm-import -- --area ...`, so the result is what the command line gives.
fn generate(name: String) {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    let dir = worlds_dir().join(&name);
    let (area, out) = (dir.join("area.json"), dir.join("world.msgpack"));
    // Never a sibling binary: next to a debug editor that is an unoptimised, possibly stale importer,
    // and decoding the laser data that way is many times slower. Cargo rebuilds the release one only
    // when the source changed.
    let mut command = match std::env::var("RAGNAROK_IMPORTER").ok().filter(|path| !path.is_empty()) {
        Some(binary) => Command::new(binary),
        None => {
            let mut command = Command::new("cargo");
            command.args(["run", "-q", "--release", "-p", "osm-import", "--"]);
            command
        }
    };
    command.arg("--area").arg(&area).arg("--out").arg(&out).env("LM_USER", credential("LM_USER").unwrap_or_default());
    if let Some(password) = credential("LM_PASS") {
        command.env("LM_PASS", password);
    }
    if credential("LM_USER").is_none() {
        command.env_remove("LM_USER");
    }
    command.arg("--cache").arg(std::env::var("RAGNAROK_CACHE").unwrap_or_else(|_| ".cache/osm-import".into())).arg("--progress");
    let finish = |state: &'static str, note: String| {
        let mut job = job();
        job.state = state;
        job.done = usize::from(state == "done");
        job.ended = Some(Instant::now());
        job.messages.push(note);
    };
    let mut child = match command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::piped()).spawn() {
        Ok(child) => child,
        Err(error) => return finish("failed", format!("could not start the importer: {error}")),
    };
    if let Some(stderr) = child.stderr.take() {
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            if let Some(json) = line.strip_prefix(progress::PREFIX) {
                if let Ok(snapshot) = serde_json::from_str(json) {
                    job().progress = snapshot;
                }
                continue;
            }
            let mut job = job();
            job.messages.push(line);
            let excess = job.messages.len().saturating_sub(30);
            job.messages.drain(..excess);
        }
    }
    match child.wait() {
        Ok(status) if status.success() => finish("done", format!("wrote {}", out.display())),
        Ok(status) => finish("failed", format!("the importer stopped: {status}")),
        Err(error) => finish("failed", format!("the importer could not be waited for: {error}")),
    }
}

#[handler]
async fn start_generate(req: &mut Request, res: &mut Response) {
    let Some(name) = name_of(req, res) else { return };
    if read_area(&name).is_none() {
        return fail(res, StatusCode::NOT_FOUND, "no such world");
    }
    {
        let mut current = job();
        if current.state == "running" {
            return fail(res, StatusCode::CONFLICT, "a job is already running");
        }
        *current = Job { world: name.clone(), kind: "generate", state: "running", total: 1, started: Some(Instant::now()), ..Job::default() };
    }
    std::thread::spawn(move || generate(name));
    res.render(Json(current()));
}

#[handler]
async fn job_state(res: &mut Response) {
    res.render(Json(current()));
}

/// The job as the API reports it. A download's progress is read live from this process; a
/// generate's arrives as `@progress` lines from the importer, and is already in the job.
fn current() -> Job {
    let mut job = job().clone();
    if job.kind == "download" && job.state == "running" {
        job.progress = progress::snapshot();
    }
    let now = Instant::now();
    job.elapsed_ms = job.started.map_or(0, |started| job.ended.unwrap_or(now).duration_since(started).as_millis() as u64);
    job
}

/// A credential from the environment, else from `.env.local` (as the importer's script does).
fn credential(key: &str) -> Option<String> {
    std::env::var(key).ok().or_else(|| {
        let text = std::fs::read_to_string(".env.local").ok()?;
        text.lines().filter_map(|line| line.split_once('=')).find(|(k, _)| k.trim() == key).map(|(_, v)| v.trim().trim_matches('"').to_string())
    })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    cache::init(Some(std::env::var("RAGNAROK_CACHE").map_or_else(|_| PathBuf::from(".cache/osm-import"), PathBuf::from)), false);
    let router = Router::with_path("api")
        .push(Router::with_path("grid").get(grid_info))
        .push(Router::with_path("cache").get(cache_state))
        .push(Router::with_path("job").get(job_state))
        .push(Router::with_path("worlds").get(list_worlds))
        .push(
            Router::with_path("worlds/{name}")
                .get(get_world)
                .put(put_world)
                .delete(delete_world)
                .push(Router::with_path("fetch").post(start_fetch))
                .push(Router::with_path("generate").post(start_generate))
                .push(Router::with_path("activate").post(activate)),
        );
    let acceptor = TcpListener::new(("127.0.0.1", 8700)).bind().await;
    eprintln!("map editor API on http://127.0.0.1:8700/api (run the client with `npm run dev` and open /editor.html)");
    Server::new(acceptor).serve(router).await;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_saved_area_keeps_its_bots_and_their_routes() {
        let sent = r#"{"tiles":[{"gx":1,"gy":2}],"spawns":[],"vehicles":[],"items":[],
            "bots":[{"lat":63.82,"lon":20.29,"team":2,"skill":0.8,"route":[{"lat":63.82,"lon":20.29},{"lat":63.83,"lon":20.3}]}]}"#;
        let area: Area = serde_json::from_str(sent).unwrap();
        let saved: serde_json::Value = serde_json::from_slice(&serde_json::to_vec(&area).unwrap()).unwrap();
        let bot = &saved["bots"][0];
        assert_eq!((bot["team"].as_u64(), bot["skill"].as_f64()), (Some(2), Some(0.8)));
        assert_eq!(bot["route"].as_array().map(Vec::len), Some(2));
        // An area saved before bots could be placed still loads.
        let old: Area = serde_json::from_str(r#"{"tiles":[]}"#).unwrap();
        assert!(old.bots.is_empty());
    }
}
