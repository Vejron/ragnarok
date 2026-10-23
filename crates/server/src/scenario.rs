//! Scenarios: what happens in a world, read from its `scenario.json` (see BOTS.md). A scenario has
//! named areas, squads of bots and triggers. A trigger fires when all of its conditions come true
//! (someone enters an area, a squad is cleared, time has passed, another trigger is done), and then
//! does its actions: spawns or removes a squad, tells everyone something, sets the objective, or
//! ends the scenario won or lost. After the end it starts again from the top.
//!
//! The file names things; `Scenario` holds them as indices, checked once when it is read, so a
//! typo stops the server with a message instead of a trigger that never fires.

use crate::game::{Bot, Game, Player, Seats, Vehicle, World, Wreck};
use crate::entities::With;
use crate::mechanics::{position_of, remove_vehicle};
use anyhow::{Context, Result, bail};
use serde::Deserialize;
use shared::consts::TICK_RATE;
use shared::items::{HOTBAR_SLOTS, ITEM_COUNT, ITEM_NONE, def};
use shared::protocol::ItemSlot;
use shared::protocol::{NOTICE_LOST, NOTICE_OBJECTIVE, NOTICE_SAY, NOTICE_WON};
use shared::world::{WorldFile, project};
use std::collections::BTreeMap;
use std::path::Path;

/// Longest text a notice may carry, characters.
const MAX_TEXT: usize = 200;

// The file, as written.

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FileSpec {
    #[serde(default)]
    areas: BTreeMap<String, AreaSpec>,
    #[serde(default)]
    squads: BTreeMap<String, SquadSpec>,
    #[serde(default)]
    triggers: Vec<TriggerSpec>,
    /// Seconds from the end until it starts again.
    #[serde(default = "restart")]
    restart: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AreaSpec {
    lat: f64,
    lon: f64,
    radius: f32,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SquadSpec {
    #[serde(default = "one")]
    team: u8,
    #[serde(default = "half")]
    skill: f32,
    /// Item names, one a slot; none is the standard loadout.
    #[serde(default)]
    loadout: Option<Vec<String>>,
    /// Whether its bots come back at home after dying, as placed bots do. Then it is never cleared
    /// unless they are all dead at once.
    #[serde(default)]
    respawn: bool,
    #[serde(default)]
    bots: Vec<BotSpec>,
    /// Tanks, each with a crew of two of the squad's bots. A tank's route is driven, and with none
    /// it drives along the nearest road.
    #[serde(default)]
    tanks: Vec<BotSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct BotSpec {
    lat: f64,
    lon: f64,
    #[serde(default)]
    route: Vec<SpotSpec>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SpotSpec {
    lat: f64,
    lon: f64,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TriggerSpec {
    #[serde(default)]
    name: Option<String>,
    when: WhenSpec,
    /// Seconds between its conditions coming true and its actions.
    #[serde(default)]
    wait: f32,
    /// How many times it may fire; 0 is no limit.
    #[serde(default = "one_u32")]
    times: u32,
    #[serde(rename = "do")]
    actions: Vec<ActionSpec>,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct WhenSpec {
    #[serde(default)]
    start: bool,
    /// Seconds since the scenario started.
    #[serde(default)]
    after: Option<f32>,
    /// An area someone is in: a person, or with `by`, one of that squad.
    #[serde(default)]
    enter: Option<String>,
    #[serde(default)]
    by: Option<String>,
    /// A squad that has been spawned and has no one left standing.
    #[serde(default)]
    cleared: Option<String>,
    /// A trigger that has fired all its times (or, with no limit, at least once).
    #[serde(default)]
    done: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "lowercase")]
enum ActionSpec {
    Spawn(String),
    Remove(String),
    Say(String),
    Objective(String),
    Win(String),
    Lose(String),
}

fn one() -> u8 {
    1
}

fn one_u32() -> u32 {
    1
}

fn half() -> f32 {
    0.5
}

fn restart() -> f32 {
    20.0
}

// The scenario, checked.

/// A circle on the map, metres.
struct Area {
    at: [f32; 2],
    radius: f32,
}

struct Squad {
    name: String,
    team: u8,
    skill: f32,
    loadout: Option<[ItemSlot; HOTBAR_SLOTS]>,
    respawn: bool,
    /// Each bot's home and route, `[x, z]` metres, and each tank's.
    bots: Vec<([f32; 2], Vec<[f32; 2]>)>,
    tanks: Vec<([f32; 2], Vec<[f32; 2]>)>,
}

#[derive(Debug, PartialEq)]
enum Condition {
    Start,
    After(u32),
    Enter { area: usize, by: Option<usize> },
    Cleared(usize),
    Done(usize),
}

#[derive(Debug, PartialEq)]
enum Action {
    Spawn(usize),
    Remove(usize),
    Say(String),
    Objective(String),
    Win(String),
    Lose(String),
}

struct Trigger {
    name: Option<String>,
    when: Vec<Condition>,
    wait: u32,
    times: u32,
    actions: Vec<Action>,
}

/// Where a run of the scenario has got to. A fresh one is the start.
#[derive(Default)]
struct Progress {
    /// Ticks since it started.
    ticks: u32,
    /// For each trigger: how often it has fired, whether its conditions held last tick, and the
    /// ticks left before its actions when it is waiting to fire.
    fired: Vec<u32>,
    held: Vec<bool>,
    waiting: Vec<Option<u32>>,
    /// For each squad: its bots, its tanks with the crew each came with, and whether it has ever
    /// been spawned.
    members: Vec<Vec<u32>>,
    tanks: Vec<Vec<(u32, [u32; 2])>>,
    spawned: Vec<bool>,
    objective: String,
    /// Ticks left until it starts again, once it has ended.
    ended: Option<u32>,
}

pub struct Scenario {
    areas: Vec<Area>,
    squads: Vec<Squad>,
    triggers: Vec<Trigger>,
    restart: u32,
    progress: Progress,
}

fn ticks(seconds: f32) -> u32 {
    (seconds.max(0.0) * TICK_RATE as f32).round() as u32
}

/// The hotbar for `names`, a slot each, every gun loaded and every stack full.
fn loadout(names: &[String]) -> Result<[ItemSlot; HOTBAR_SLOTS]> {
    if names.len() > HOTBAR_SLOTS {
        bail!("a loadout has at most {HOTBAR_SLOTS} items, not {}", names.len());
    }
    let mut hotbar = [ItemSlot { item: ITEM_NONE, count: 0, ammo: 0 }; HOTBAR_SLOTS];
    for (slot, name) in hotbar.iter_mut().zip(names) {
        let Some((item, d)) = (ITEM_NONE + 1..ITEM_COUNT).filter_map(|item| def(item).map(|d| (item, d))).find(|(_, d)| d.name.eq_ignore_ascii_case(name)) else {
            let known: Vec<_> = (ITEM_NONE + 1..ITEM_COUNT).filter_map(|item| def(item).map(|d| d.name.to_lowercase())).collect();
            bail!("no item called {name:?}; there are {}", known.join(", "));
        };
        *slot = ItemSlot { item, count: d.stack.max(1), ammo: d.mag };
    }
    Ok(hotbar)
}

fn short(text: String) -> Result<String> {
    if text.chars().count() > MAX_TEXT {
        bail!("{text:?} is longer than {MAX_TEXT} characters");
    }
    Ok(text)
}

/// The scenario in the world's `scenario.json` at `path`, or `None` when there is none.
pub fn load(path: &Path, map: &WorldFile) -> Result<Option<Scenario>> {
    let text = match std::fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("reading {}", path.display())),
    };
    let scenario = read(&text, map).with_context(|| format!("in {}", path.display()))?;
    tracing::info!(path = %path.display(), areas = scenario.areas.len(), squads = scenario.squads.len(), triggers = scenario.triggers.len(), "scenario loaded");
    Ok(Some(scenario))
}

/// The scenario written in `text`, on `map`.
pub fn read(text: &str, map: &WorldFile) -> Result<Scenario> {
    Scenario::parse(text, |lat, lon| {
        let [x, z] = project(map.origin, lat, lon);
        map.is_inside((x * 100.0).round() as i32, (z * 100.0).round() as i32).then_some([x as f32, z as f32])
    })
}

impl Scenario {
    /// The scenario written in `text`, placed on the map by `locate` (latitude, longitude to `[x, z]`
    /// metres, `None` outside the world).
    pub fn parse(text: &str, locate: impl Fn(f64, f64) -> Option<[f32; 2]>) -> Result<Self> {
        let file: FileSpec = serde_json::from_str(text)?;
        let place = |what: &str, lat: f64, lon: f64| locate(lat, lon).with_context(|| format!("{what} at {lat}, {lon} is outside the world"));
        let area_names: Vec<String> = file.areas.keys().cloned().collect();
        let squad_names: Vec<String> = file.squads.keys().cloned().collect();
        let trigger_names: Vec<String> = file.triggers.iter().filter_map(|t| t.name.clone()).collect();
        let find = |kind: &str, names: &[String], name: &str| {
            names.iter().position(|n| n == name).with_context(|| format!("there is no {kind} called {name:?}"))
        };
        for (n, name) in trigger_names.iter().enumerate() {
            if trigger_names[..n].contains(name) {
                bail!("two triggers are called {name:?}");
            }
        }

        let areas = file
            .areas
            .iter()
            .map(|(name, a)| Ok(Area { at: place(&format!("area {name:?}"), a.lat, a.lon)?, radius: a.radius }))
            .collect::<Result<_>>()?;
        let squads = file
            .squads
            .into_iter()
            .map(|(name, s)| {
                let spots = |what: String, specs: &[BotSpec]| {
                    specs
                        .iter()
                        .map(|b| Ok((place(&what, b.lat, b.lon)?, b.route.iter().map(|p| place(&format!("a waypoint of {what}"), p.lat, p.lon)).collect::<Result<_>>()?)))
                        .collect::<Result<Vec<_>>>()
                };
                let bots = spots(format!("a bot of squad {name:?}"), &s.bots)?;
                let tanks = spots(format!("a tank of squad {name:?}"), &s.tanks)?;
                if bots.is_empty() && tanks.is_empty() {
                    bail!("squad {name:?} has no bots and no tanks");
                }
                let loadout = s.loadout.as_deref().map(loadout).transpose().with_context(|| format!("squad {name:?}"))?;
                Ok(Squad { name, team: s.team, skill: s.skill.clamp(0.0, 1.0), loadout, respawn: s.respawn, bots, tanks })
            })
            .collect::<Result<_>>()?;

        // Indices into the triggers that have a name.
        let named: Vec<usize> = file.triggers.iter().enumerate().filter(|(_, t)| t.name.is_some()).map(|(i, _)| i).collect();
        let mut triggers = Vec::new();
        for (n, t) in file.triggers.into_iter().enumerate() {
            let what = t.name.clone().map_or_else(|| format!("trigger {}", n + 1), |name| format!("trigger {name:?}"));
            let w = t.when;
            let mut when = Vec::new();
            if w.start {
                when.push(Condition::Start);
            }
            if let Some(seconds) = w.after {
                when.push(Condition::After(ticks(seconds)));
            }
            let by = w.by.as_deref().map(|name| find("squad", &squad_names, name)).transpose().with_context(|| what.clone())?;
            match &w.enter {
                Some(area) => when.push(Condition::Enter { area: find("area", &area_names, area).with_context(|| what.clone())?, by }),
                None if by.is_some() => bail!("{what}: \"by\" goes with \"enter\""),
                None => {}
            }
            if let Some(squad) = &w.cleared {
                when.push(Condition::Cleared(find("squad", &squad_names, squad).with_context(|| what.clone())?));
            }
            if let Some(trigger) = &w.done {
                when.push(Condition::Done(named[find("trigger", &trigger_names, trigger).with_context(|| what.clone())?]));
            }
            if when.is_empty() {
                bail!("{what}: \"when\" needs at least one of start, after, enter, cleared and done");
            }
            let actions = t
                .actions
                .into_iter()
                .map(|a| {
                    Ok(match a {
                        ActionSpec::Spawn(squad) => Action::Spawn(find("squad", &squad_names, &squad)?),
                        ActionSpec::Remove(squad) => Action::Remove(find("squad", &squad_names, &squad)?),
                        ActionSpec::Say(s) => Action::Say(short(s)?),
                        ActionSpec::Objective(s) => Action::Objective(short(s)?),
                        ActionSpec::Win(s) => Action::Win(short(s)?),
                        ActionSpec::Lose(s) => Action::Lose(short(s)?),
                    })
                })
                .collect::<Result<_>>()
                .with_context(|| what.clone())?;
            triggers.push(Trigger { name: t.name, when, wait: ticks(t.wait), times: t.times, actions });
        }
        let mut scenario = Self { areas, squads, triggers, restart: ticks(file.restart), progress: Progress::default() };
        scenario.progress = scenario.fresh();
        Ok(scenario)
    }

    fn fresh(&self) -> Progress {
        let (t, s) = (self.triggers.len(), self.squads.len());
        Progress { fired: vec![0; t], held: vec![false; t], waiting: vec![None; t], members: vec![Vec::new(); s], tanks: vec![Vec::new(); s], spawned: vec![false; s], ..Default::default() }
    }

    /// What everyone is meant to be doing now; empty when nothing.
    pub fn objective(&self) -> &str {
        &self.progress.objective
    }

    /// Whether it has ended, won or lost, and is waiting to start again.
    pub fn ended(&self) -> bool {
        self.progress.ended.is_some()
    }

    /// One tick: the bots that fell for good leave, the triggers whose conditions just came true
    /// start waiting, and those done waiting fire. Once it has ended it waits `restart`, then clears
    /// its bots away and starts again.
    pub fn tick(&mut self, game: &mut Game) {
        // Even once it has ended: a squad that does not respawn must not come back while it waits.
        self.clear_fallen(game);
        self.clear_tanks(game);
        if let Some(left) = &mut self.progress.ended {
            *left = left.saturating_sub(1);
            if *left == 0 {
                tracing::info!("scenario starts again");
                for id in self.progress.members.iter().flatten() {
                    game.remove_bot(*id);
                }
                for &(vid, _) in self.progress.tanks.iter().flatten() {
                    remove_vehicle(&mut game.world, vid);
                }
                self.progress = self.fresh();
                game.notice(NOTICE_OBJECTIVE, "");
            }
            return;
        }
        for i in 0..self.triggers.len() {
            let trigger = &self.triggers[i];
            let holds = trigger.when.iter().all(|condition| self.holds(&game.world, condition));
            let rose = holds && !self.progress.held[i];
            self.progress.held[i] = holds;
            let more = trigger.times == 0 || self.progress.fired[i] < trigger.times;
            if rose && more && self.progress.waiting[i].is_none() {
                self.progress.waiting[i] = Some(trigger.wait);
            }
        }
        for i in 0..self.triggers.len() {
            match self.progress.waiting[i] {
                Some(0) => {
                    self.progress.waiting[i] = None;
                    self.progress.fired[i] += 1;
                    tracing::info!(trigger = self.triggers[i].name.as_deref().unwrap_or(""), n = i + 1, "scenario trigger fired");
                    for a in 0..self.triggers[i].actions.len() {
                        self.act(game, i, a);
                        if self.ended() {
                            return;
                        }
                    }
                }
                Some(left) => self.progress.waiting[i] = Some(left - 1),
                None => {}
            }
        }
        self.progress.ticks += 1;
    }

    /// Bots of squads that do not respawn leave once they would have: their bodies lie for as long
    /// as anyone's.
    fn clear_fallen(&mut self, game: &mut Game) {
        for (squad, members) in self.squads.iter().zip(&mut self.progress.members) {
            if squad.respawn {
                continue;
            }
            members.retain(|&id| {
                let stays = game.world.entities.get::<Player>(id).is_some_and(|p| p.alive() || p.respawn_in > 1);
                if !stays {
                    game.remove_bot(id);
                }
                stays
            });
        }
    }

    /// A squad's tank goes once its crew have: when no one is in it, and, if it is a wreck, once it
    /// has burnt for as long as any wreck does.
    fn clear_tanks(&mut self, game: &mut Game) {
        for tanks in &mut self.progress.tanks {
            tanks.retain(|&(vid, crew)| {
                let world = &game.world;
                if !world.entities.has::<Vehicle>(vid) {
                    return false;
                }
                let crew_gone = crew.iter().all(|id| !world.entities.has::<Player>(*id));
                let empty = world.entities.get::<Seats>(vid).is_none_or(Seats::vacant);
                let burnt = !world.entities.dead(vid) || world.entities.get::<Wreck>(vid).is_none_or(|wreck| wreck.left <= 1);
                let goes = crew_gone && empty && burnt;
                if goes {
                    remove_vehicle(&mut game.world, vid);
                }
                !goes
            });
        }
    }

    fn holds(&self, world: &World, condition: &Condition) -> bool {
        let progress = &self.progress;
        match *condition {
            Condition::Start => true,
            Condition::After(ticks) => progress.ticks >= ticks,
            Condition::Enter { area, by } => {
                let Area { at, radius } = self.areas[area];
                let inside = |id: &u32| {
                    world.entities.get::<Player>(*id).is_some_and(|p| p.alive()) && position_of(world, *id).is_some_and(|p| (p[0] - at[0]).hypot(p[2] - at[1]) <= radius)
                };
                match by {
                    Some(squad) => progress.members[squad].iter().any(inside),
                    None => world.entities.ids::<With<Player>>().iter().filter(|id| !world.entities.has::<Bot>(**id)).any(inside),
                }
            }
            Condition::Cleared(squad) => progress.spawned[squad] && !progress.members[squad].iter().any(|id| world.entities.get::<Player>(*id).is_some_and(|p| p.alive())),
            Condition::Done(trigger) => {
                let (times, fired) = (self.triggers[trigger].times, progress.fired[trigger]);
                fired >= times.max(1)
            }
        }
    }

    /// Does action `a` of trigger `t`.
    fn act(&mut self, game: &mut Game, t: usize, a: usize) {
        match &self.triggers[t].actions[a] {
            &Action::Spawn(s) => {
                let squad = &self.squads[s];
                let mut name = squad.name.clone();
                if let Some(first) = name.get_mut(..1) {
                    first.make_ascii_uppercase();
                }
                for (n, (home, route)) in squad.bots.iter().enumerate() {
                    let Some(id) = game.add_bot_as(format!("{name} {}", n + 1), *home, squad.team, route.clone()) else {
                        tracing::warn!(squad = squad.name, "MAX_BOTS reached, squad not all spawned");
                        break;
                    };
                    if let Some(mut bot) = game.world.entities.get_mut::<Bot>(id) {
                        bot.skill = squad.skill;
                        bot.loadout = squad.loadout;
                    }
                    if let (Some(hotbar), Some(mut player)) = (squad.loadout, game.world.entities.get_mut::<Player>(id)) {
                        player.hotbar = hotbar;
                    }
                    self.progress.members[s].push(id);
                }
                let mut n = squad.bots.len();
                for (at, route) in &squad.tanks {
                    let names = [format!("{name} {}", n + 1), format!("{name} {}", n + 2)];
                    let Some((vid, crew)) = game.add_crew(names, *at, squad.team, route.clone()) else {
                        tracing::warn!(squad = squad.name, "no room for a tank or its crew; not spawned");
                        continue;
                    };
                    n += 2;
                    for id in crew {
                        if let Some(mut bot) = game.world.entities.get_mut::<Bot>(id) {
                            bot.skill = squad.skill;
                            bot.loadout = squad.loadout;
                        }
                        if let (Some(hotbar), Some(mut player)) = (squad.loadout, game.world.entities.get_mut::<Player>(id)) {
                            player.hotbar = hotbar;
                        }
                    }
                    self.progress.members[s].extend(crew);
                    self.progress.tanks[s].push((vid, crew));
                }
                self.progress.spawned[s] = true;
            }
            &Action::Remove(s) => {
                for id in std::mem::take(&mut self.progress.members[s]) {
                    game.remove_bot(id);
                }
                for (vid, _) in std::mem::take(&mut self.progress.tanks[s]) {
                    remove_vehicle(&mut game.world, vid);
                }
            }
            Action::Say(text) => game.notice(NOTICE_SAY, text),
            Action::Objective(text) => {
                self.progress.objective = text.clone();
                game.notice(NOTICE_OBJECTIVE, text);
            }
            Action::Win(text) | Action::Lose(text) => {
                let kind = if matches!(self.triggers[t].actions[a], Action::Win(_)) { NOTICE_WON } else { NOTICE_LOST };
                tracing::info!(won = kind == NOTICE_WON, text, "scenario ended");
                game.notice(kind, text);
                self.progress.objective.clear();
                self.progress.ended = Some(self.restart.max(1));
            }
        }
    }
}

#[cfg(test)]
mod tests;
