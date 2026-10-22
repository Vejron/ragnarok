use super::*;
use crate::game::{Command, Outgoing, Profile};
use shared::consts::{RESPAWN_TICKS, WRECK_TICKS};
use shared::items::{ITEM_GRENADE, ITEM_NONE, ITEM_SHOTGUN, ITEM_SNIPER};
use shared::protocol::{Notice, S_NOTICE, decode, peek_tag};
use shared::world::{METRES_PER_DEG_LAT, WORLD_VERSION, metres_per_deg_lon};
use tokio::sync::{mpsc, oneshot};

/// `"lat": …, "lon": …` for `[x, z]` metres on a world centred on 0, 0.
fn ll(x: f32, z: f32) -> String {
    format!(r#""lat": {}, "lon": {}"#, -f64::from(z) / METRES_PER_DEG_LAT, f64::from(x) / metres_per_deg_lon())
}

fn map() -> WorldFile {
    WorldFile { version: WORLD_VERSION, half_extent: 10_000, spawns: vec![[-8_000, -8_000]], ..Default::default() }
}

/// A flat 200 m square running the scenario in `json`. People spawn at its far corner, -80, -80.
fn start(json: &str) -> Game {
    let mut game = Game::new(map(), 0).unwrap();
    game.world.clear_pickups();
    game.scenario = Some(read(json, &game.world.map).unwrap());
    game
}

fn join(game: &mut Game) -> (u32, mpsc::Receiver<Outgoing>) {
    let (outbox, inbox) = mpsc::channel(4096);
    let (reply, mut response) = oneshot::channel();
    game.handle(Command::Join { outbox, reply, profile: Profile::default() });
    (response.try_recv().unwrap().unwrap(), inbox)
}

/// The notices sent since the last look, as `(kind, text)`.
fn notices(inbox: &mut mpsc::Receiver<Outgoing>) -> Vec<(u8, String)> {
    std::iter::from_fn(|| inbox.try_recv().ok())
        .filter_map(|out| match out {
            Outgoing::Reliable(bytes) if peek_tag(&bytes) == Some(S_NOTICE) => decode::<Notice>(&bytes).map(|Notice(_, kind, text)| (kind, text)),
            _ => None,
        })
        .collect()
}

fn run(game: &mut Game, seconds: f32) {
    for _ in 0..(seconds * TICK_RATE as f32).round() as u32 {
        game.step();
    }
}

fn bots(game: &Game) -> Vec<u32> {
    game.world.bots.keys().copied().collect()
}

fn alive_bots(game: &Game) -> Vec<u32> {
    bots(game).into_iter().filter(|id| game.world.entities.of::<Player>(*id).alive()).collect()
}

/// Kills player `id` as anything would: down to nothing, and back after the usual wait.
fn kill(game: &mut Game, id: u32) {
    let mut player = game.world.entities.get_mut::<Player>(id).unwrap();
    player.hp = 0;
    player.respawn_in = RESPAWN_TICKS;
}

fn put(game: &mut Game, id: u32, [x, z]: [f32; 2]) {
    let y = game.world.map.ground.height_at(x, z) + sim::STANDING_Y;
    game.world.entities.get_mut::<Player>(id).unwrap().character.position = [x, y, z];
}

fn error(json: &str) -> String {
    format!("{:#}", read(json, &map()).err().expect("refused"))
}

#[test]
fn a_squad_spawns_at_the_start_with_its_loadout_and_everyone_hears_the_objective() {
    let json = format!(
        r#"{{
            "squads": {{ "guards": {{ "team": 2, "skill": 0.9, "loadout": ["shotgun", "Grenade"], "bots": [ {{ {} }}, {{ {} }} ] }} }},
            "triggers": [ {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "guards" }}, {{ "objective": "Clear the farm" }} ] }} ]
        }}"#,
        ll(60.0, 60.0),
        ll(65.0, 60.0)
    );
    let mut game = start(&json);
    let (_, mut early) = join(&mut game);
    game.step();
    assert_eq!(notices(&mut early), vec![(NOTICE_OBJECTIVE, "Clear the farm".into())]);

    let ids = bots(&game);
    assert_eq!(ids.len(), 2);
    let names: Vec<_> = ids.iter().map(|id| game.world.entities.of::<Player>(*id).name.clone()).collect();
    assert_eq!(names, ["Guards 1", "Guards 2"]);
    for id in &ids {
        let (player, bot) = (&game.world.entities.of::<Player>(*id), &game.world.bots[id]);
        assert_eq!(player.team, 2);
        assert_eq!(bot.skill, 0.9);
        assert_eq!(player.hotbar[0].item, ITEM_SHOTGUN);
        assert_eq!(player.hotbar[0].ammo, def(ITEM_SHOTGUN).unwrap().mag, "loaded");
        assert_eq!((player.hotbar[1].item, player.hotbar[1].count), (ITEM_GRENADE, def(ITEM_GRENADE).unwrap().stack), "a full stack");
        assert_eq!(player.hotbar[2].item, ITEM_NONE, "nothing else");
    }

    run(&mut game, 1.0);
    let (_, mut late) = join(&mut game);
    assert_eq!(notices(&mut late), vec![(NOTICE_OBJECTIVE, "Clear the farm".into())], "a joiner is told what is going on");
    assert!(notices(&mut early).is_empty(), "and the objective is not said again");
}

#[test]
fn a_fallen_bot_lies_as_long_as_anyone_would_and_then_leaves() {
    let json = format!(
        r#"{{ "squads": {{ "guards": {{ "bots": [ {{ {} }} ] }} }}, "triggers": [ {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "guards" }} ] }} ] }}"#,
        ll(60.0, 60.0)
    );
    let mut game = start(&json);
    game.step();
    let [id] = bots(&game)[..] else { panic!("one bot") };
    kill(&mut game, id);
    run(&mut game, RESPAWN_TICKS as f32 / TICK_RATE as f32 - 0.2);
    assert!(game.world.entities.has::<Player>(id), "its body still lies there");
    run(&mut game, 0.4);
    assert!(!game.world.entities.has::<Player>(id) && bots(&game).is_empty(), "it does not come back");
}

#[test]
fn clearing_the_squad_wins_and_after_a_while_it_all_starts_again() {
    let json = format!(
        r#"{{
            "restart": 5,
            "squads": {{ "guards": {{ "bots": [ {{ {} }}, {{ {} }} ] }} }},
            "triggers": [
                {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "guards" }}, {{ "objective": "Clear the farm" }} ] }},
                {{ "when": {{ "cleared": "guards" }}, "do": [ {{ "win": "The farm is yours" }}, {{ "say": "never said" }} ] }}
            ]
        }}"#,
        ll(60.0, 60.0),
        ll(62.0, 60.0)
    );
    let mut game = start(&json);
    let (_, mut inbox) = join(&mut game);
    game.step();
    let first = bots(&game);
    kill(&mut game, first[0]);
    run(&mut game, 1.0);
    assert_eq!(notices(&mut inbox), vec![(NOTICE_OBJECTIVE, "Clear the farm".into())], "one of two down is not cleared");
    kill(&mut game, first[1]);
    game.step();
    assert_eq!(notices(&mut inbox), vec![(NOTICE_WON, "The farm is yours".into())], "and the actions after the end are not done");
    assert!(game.scenario.as_ref().unwrap().ended());

    // It starts again on the last tick of the five seconds, and the start trigger fires on the next.
    run(&mut game, 5.0);
    game.step();
    assert_eq!(notices(&mut inbox), vec![(NOTICE_OBJECTIVE, String::new()), (NOTICE_OBJECTIVE, "Clear the farm".into())], "cleared, then set again");
    let again = alive_bots(&game);
    assert_eq!(again.len(), 2, "the squad is back");
    assert!(first.iter().all(|id| !game.world.entities.has::<Player>(*id)), "the fallen were cleared away");
}

#[test]
fn coming_into_an_area_fires_each_time_someone_comes_in_not_while_they_stay() {
    let json = format!(
        r#"{{ "areas": {{ "gate": {{ {}, "radius": 10 }} }}, "triggers": [ {{ "when": {{ "enter": "gate" }}, "times": 0, "do": [ {{ "say": "Halt" }} ] }} ] }}"#,
        ll(-60.0, -60.0)
    );
    let mut game = start(&json);
    let (id, mut inbox) = join(&mut game);
    run(&mut game, 0.5);
    assert!(notices(&mut inbox).is_empty(), "outside at the spawn");
    put(&mut game, id, [-58.0, -62.0]);
    run(&mut game, 1.0);
    assert_eq!(notices(&mut inbox), vec![(NOTICE_SAY, "Halt".into())]);
    put(&mut game, id, [-80.0, -80.0]);
    game.step();
    put(&mut game, id, [-60.0, -60.0]);
    game.step();
    assert_eq!(notices(&mut inbox), vec![(NOTICE_SAY, "Halt".into())], "out and in again");
    kill(&mut game, id);
    game.step();
    put(&mut game, id, [-80.0, -80.0]);
    game.step();
    put(&mut game, id, [-60.0, -60.0]);
    game.step();
    assert!(notices(&mut inbox).is_empty(), "the dead do not count");
}

#[test]
fn waves_come_one_after_another_and_clearing_the_last_wins() {
    let json = format!(
        r#"{{
            "squads": {{ "raiders": {{ "team": 3, "bots": [ {{ {} }}, {{ {} }} ] }} }},
            "triggers": [
                {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "raiders" }} ] }},
                {{ "name": "waves", "when": {{ "cleared": "raiders" }}, "wait": 2, "times": 2, "do": [ {{ "say": "Here they come" }}, {{ "spawn": "raiders" }} ] }},
                {{ "when": {{ "done": "waves", "cleared": "raiders" }}, "do": [ {{ "win": "Held" }} ] }}
            ]
        }}"#,
        ll(70.0, 70.0),
        ll(72.0, 70.0)
    );
    let mut game = start(&json);
    let (_, mut inbox) = join(&mut game);
    let mut said = Vec::new();
    game.step();
    for wave in 0..3 {
        assert_eq!(alive_bots(&game).len(), 2, "wave {wave}");
        for id in alive_bots(&game) {
            kill(&mut game, id);
        }
        run(&mut game, 1.5);
        assert!(alive_bots(&game).is_empty(), "the next wave waits, and the fallen stay down, after wave {wave}");
        run(&mut game, 1.0);
        said.extend(notices(&mut inbox));
    }
    assert_eq!(said, vec![(NOTICE_SAY, "Here they come".into()), (NOTICE_SAY, "Here they come".into()), (NOTICE_WON, "Held".into())]);
    run(&mut game, 3.0);
    assert!(alive_bots(&game).is_empty(), "no one gets up again while it waits to start over");
}

#[test]
fn time_running_out_or_a_squad_reaching_an_area_loses() {
    let late = r#"{ "triggers": [ { "when": { "after": 3 }, "do": [ { "lose": "Out of time" } ] } ] }"#;
    let mut game = start(late);
    let (_, mut inbox) = join(&mut game);
    run(&mut game, 2.9);
    assert!(notices(&mut inbox).is_empty());
    run(&mut game, 0.2);
    assert_eq!(notices(&mut inbox), vec![(NOTICE_LOST, "Out of time".into())]);

    let taken = format!(
        r#"{{
            "areas": {{ "base": {{ {}, "radius": 8 }} }},
            "squads": {{ "raiders": {{ "bots": [ {{ {} }} ] }} }},
            "triggers": [
                {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "raiders" }} ] }},
                {{ "when": {{ "enter": "base", "by": "raiders" }}, "do": [ {{ "lose": "They took the base" }} ] }},
                {{ "when": {{ "enter": "base" }}, "do": [ {{ "say": "not by people" }} ] }}
            ]
        }}"#,
        ll(40.0, 40.0),
        ll(55.0, 40.0)
    );
    let mut game = start(&taken);
    let (_, mut inbox) = join(&mut game);
    run(&mut game, 1.0);
    assert!(notices(&mut inbox).is_empty(), "the raider is 15 m off");
    let [raider] = bots(&game)[..] else { panic!("one raider") };
    put(&mut game, raider, [44.0, 41.0]);
    game.step();
    assert_eq!(notices(&mut inbox), vec![(NOTICE_LOST, "They took the base".into())], "a bot in the area is not a person in it");
}

#[test]
fn a_squad_that_respawns_comes_back_with_its_own_loadout() {
    let json = format!(
        r#"{{ "squads": {{ "snipers": {{ "respawn": true, "loadout": ["sniper"], "bots": [ {{ {} }} ] }} }}, "triggers": [ {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "snipers" }} ] }} ] }}"#,
        ll(60.0, 60.0)
    );
    let mut game = start(&json);
    game.step();
    let [id] = bots(&game)[..] else { panic!("one bot") };
    kill(&mut game, id);
    run(&mut game, RESPAWN_TICKS as f32 / TICK_RATE as f32 + 0.5);
    let player = &game.world.entities.of::<Player>(id);
    assert!(player.alive(), "back");
    assert_eq!(player.hotbar[0].item, ITEM_SNIPER);
    assert_eq!(player.hotbar[1].item, ITEM_NONE, "not the standard loadout");
}

#[test]
fn mistakes_in_the_file_are_refused_with_what_is_wrong() {
    let cases = [
        (r#"{ "triggers": [ { "when": { "cleared": "nobody" }, "do": [] } ] }"#.to_string(), r#"no squad called "nobody""#),
        (r#"{ "triggers": [ { "when": { "entre": "gate" }, "do": [] } ] }"#.to_string(), "unknown field `entre`"),
        (r#"{ "triggers": [ { "when": { "start": true }, "do": [ { "spawn": "nobody" } ] } ] }"#.to_string(), r#"trigger 1: there is no squad called "nobody""#),
        (r#"{ "triggers": [ { "when": { "start": true }, "do": [ { "shout": "x" } ] } ] }"#.to_string(), "unknown variant `shout`"),
        (r#"{ "triggers": [ { "when": {}, "do": [] } ] }"#.to_string(), "needs at least one"),
        (r#"{ "squads": { "s": { "bots": [ { "lat": 0, "lon": 0 } ] } }, "triggers": [ { "when": { "start": true, "by": "s" }, "do": [] } ] }"#.to_string(), r#""by" goes with "enter""#),
        (format!(r#"{{ "areas": {{ "far": {{ {}, "radius": 5 }} }} }}"#, ll(500.0, 0.0)), r#"area "far" at"#),
        (r#"{ "squads": { "s": { "loadout": ["bazooka"], "bots": [ { "lat": 0, "lon": 0 } ] } } }"#.to_string(), r#"no item called "bazooka""#),
        (r#"{ "squads": { "empty": {} } }"#.to_string(), r#"squad "empty" has no bots and no tanks"#),
        (r#"{ "triggers": [ { "name": "a", "when": { "start": true }, "do": [] }, { "name": "a", "when": { "start": true }, "do": [] } ] }"#.to_string(), r#"two triggers are called "a""#),
    ];
    for (json, wanted) in cases {
        let got = error(&json);
        assert!(got.contains(wanted), "{json}\n  said {got:?}, not {wanted:?}");
    }
}

/// The example under "Scenarios" in BOTS.md.
fn documented_example() -> &'static str {
    let doc = include_str!("../../../../BOTS.md");
    let from = doc.find("## Scenarios").expect("a Scenarios section");
    let start = from + doc[from..].find("```json\n").expect("an example") + "```json\n".len();
    &doc[start..start + doc[start..].find("```").unwrap()]
}

#[test]
fn the_example_in_bots_md_plays_through_to_a_win() {
    // A flat 600 m square round the example's farm, with people starting well away from it.
    let map = WorldFile { version: WORLD_VERSION, half_extent: 30_000, origin: (63.8420, 20.3440), spawns: vec![[-25_000, 25_000]], ..Default::default() };
    let mut game = Game::new(map, 0).unwrap();
    game.world.clear_pickups();
    game.scenario = Some(read(documented_example(), &game.world.map).unwrap());
    let (_, mut inbox) = join(&mut game);
    game.step();
    let guards = alive_bots(&game);
    assert_eq!(guards.len(), 2);
    for id in guards {
        kill(&mut game, id);
    }
    run(&mut game, 10.5);
    for wave in 0..3 {
        let raiders = alive_bots(&game);
        assert_eq!(raiders.len(), 2, "wave {wave}");
        assert_eq!(game.world.entities.of::<Player>(raiders[0]).name, "Raiders 1");
        for id in raiders {
            kill(&mut game, id);
        }
        run(&mut game, 5.5);
    }
    let said = notices(&mut inbox);
    assert_eq!(
        said,
        vec![
            (NOTICE_OBJECTIVE, "Take the farm".into()),
            (NOTICE_OBJECTIVE, "Hold the barn".into()),
            (NOTICE_SAY, "Raiders from the north!".into()),
            (NOTICE_SAY, "More of them!".into()),
            (NOTICE_SAY, "More of them!".into()),
            (NOTICE_WON, "The farm is yours".into()),
        ]
    );
}

#[test]
fn no_file_is_no_scenario_and_a_bad_one_says_which_file() {
    let dir = std::env::temp_dir().join(format!("ragnarok-scenario-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("scenario.json");
    assert!(load(&path, &map()).unwrap().is_none());
    std::fs::write(&path, r#"{ "triggers": [ { "when": { "start": true }, "do": [ { "say": "Hello" } ] } ] }"#).unwrap();
    assert!(load(&path, &map()).unwrap().is_some());
    std::fs::write(&path, "{ \"squads\": 3 }").unwrap();
    let error = format!("{:#}", load(&path, &map()).err().unwrap());
    std::fs::remove_dir_all(&dir).unwrap();
    assert!(error.contains("scenario.json"), "{error}");
}

#[test]
fn a_squad_tank_comes_crewed_and_goes_once_it_is_wrecked_and_burnt_out() {
    let json = format!(
        r#"{{
            "squads": {{ "armour": {{ "team": 3, "skill": 0.8, "bots": [ {{ {} }} ], "tanks": [ {{ {}, "route": [ {{ {} }}, {{ {} }} ] }} ] }} }},
            "triggers": [
                {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "armour" }} ] }},
                {{ "when": {{ "cleared": "armour" }}, "do": [ {{ "win": "The column is stopped" }} ] }}
            ]
        }}"#,
        ll(60.0, 60.0),
        ll(0.0, 40.0),
        ll(0.0, 40.0),
        ll(0.0, -40.0)
    );
    let mut game = start(&json);
    let (_, mut inbox) = join(&mut game);
    game.step();
    let tanks: Vec<u32> = game.world.entities.all::<Vehicle>().filter(|(_, v)| v.kind == shared::protocol::KIND_TANK).map(|(id, _)| id).collect();
    let [vid] = tanks[..] else { panic!("one tank: {tanks:?}") };
    let mut names: Vec<_> = bots(&game).iter().map(|id| game.world.entities.of::<Player>(*id).name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Armour 1", "Armour 2", "Armour 3"], "the crew are numbered after the squad's bots");
    let crew: Vec<u32> = game.world.entities.of::<Seats>(vid).iter().flatten().copied().collect();
    assert_eq!(crew.len(), 2, "both seats manned");
    for id in &crew {
        assert_eq!((game.world.entities.of::<Player>(*id).team, game.world.bots[id].skill), (3, 0.8));
    }

    crate::mechanics::damage_vehicle(&mut game.world, vid, 255, None);
    let [walker] = bots(&game).into_iter().filter(|id| !crew.contains(id)).collect::<Vec<_>>()[..] else { panic!() };
    kill(&mut game, walker);
    run(&mut game, 5.0);
    assert!(crew.iter().all(|id| !game.world.entities.has::<Player>(*id)), "the crew died in it and left");
    assert!(game.world.entities.has::<Vehicle>(vid), "the wreck still burns");
    assert!(notices(&mut inbox).iter().any(|(kind, _)| *kind == NOTICE_WON), "the squad is cleared");
    run(&mut game, WRECK_TICKS as f32 / TICK_RATE as f32);
    assert!(!game.world.entities.has::<Vehicle>(vid), "then it is gone, not put back for anyone");
}

#[test]
fn removing_a_squad_takes_its_tank_too_and_a_restart_brings_a_fresh_one() {
    let json = format!(
        r#"{{
            "restart": 2,
            "squads": {{ "armour": {{ "tanks": [ {{ {} }} ] }} }},
            "triggers": [
                {{ "when": {{ "start": true }}, "do": [ {{ "spawn": "armour" }} ] }},
                {{ "when": {{ "after": 2 }}, "do": [ {{ "remove": "armour" }}, {{ "lose": "Gone" }} ] }}
            ]
        }}"#,
        ll(0.0, 40.0)
    );
    let mut game = start(&json);
    let tanks = |game: &Game| game.world.entities.all::<Vehicle>().map(|(_, vehicle)| vehicle).filter(|v| v.kind == shared::protocol::KIND_TANK).count();
    game.step();
    assert_eq!((tanks(&game), bots(&game).len()), (1, 2));
    run(&mut game, 2.5);
    assert_eq!((tanks(&game), bots(&game).len()), (0, 0), "removed with its crew");
    run(&mut game, 2.5);
    assert_eq!((tanks(&game), bots(&game).len()), (1, 2), "and back when it starts again");
}
