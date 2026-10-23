# Architecture migration II: the leftovers

The first migration (`MIGRATION.md`, merged to `master` at `5c755f3`) moved the server to `bevy_ecs`,
shared one simulation between server and client, put components on the wire and made a vehicle kind
one definition on each side. It left five things open. This document is their plan, rules and log.
Update it in the same commit as the work.

Branch: `refactor/ecs-migration-ii`.

## What is left, and why it matters

1. **Events.** Mechanics record what happened by pushing onto three lists on `World`: `hits`
   (`Vec<(u32, Hit)>`), `kills` (`Vec<Kill>`) and `noises` (`Vec<Noise>`). There are 16
   pushes and drains outside tests, in `combat.rs`, `npcs.rs`, `vehicles.rs`, `bots.rs` and
   `game.rs`. Each list is already a wire message (`Hit`, `Kill`) or a bot's input (`Noise`), so the
   damage code builds the client's hit marker itself. A new reaction to a death (a sound, a
   quest, a score rule) means editing `damage_hit`.
2. **A player's life is not the generic one.** Everything else uses `Health`, `Hurtbox`,
   `damage_entity` and `first_hurtbox_hit` (`mechanics/life.rs`). Players keep `Player::hp`, their
   own capsule test (`body_of`, `first_player_hit`), their own damage path (`damage`, `damage_from`,
   `damage_hit`, 11 callers) and their own respawn. `first_living_hit` asks both sides and merges
   the answers. This is the last thing in combat chosen by family. What players have that the
   generic path lacks:
   - lag compensation (`back` ticks, from `World::position_at`/`head_at`);
   - a posture-dependent body (standing capsule, or seated in the open, or none when sealed in
     armour; seen through glass for `Exposure::Glazed`);
   - headshots (`head_y`, `HEADSHOT_MULT`);
   - teams;
   - kill credit, death style and the throw of the body;
   - getting out of the seat on death;
   - the bot's `hit_from`.
3. **The per-receiver header.** `Snapshot` carries `me: SelfState` (hp, progress, hotbar) and
   `drive: Option<DriveState>` (the driven vehicle's exact motion) outside the components. They are
   sent whole every snapshot, not diffed against the baseline, and a new owner-only fact means a new
   header field on both sides.
4. **Seated players are not entities on the wire.** `game/replicate.rs` sends only embodied
   players. The client works out who sits where from the vehicle's seat list, so a rider is drawn
   by the vehicle's presenter, not as a stickman with a component.
5. **The codec's message types.** Snapshots are decoded from the generated component schema, but
   the other eleven server messages (`S_WELCOME`, `S_KILL`, `S_HIT`, `S_SCORES`, `S_DESTROYED`,
   `S_DAMAGED`, `S_CARVED`, `S_PROFILES`, `S_NOTICE`, `S_LEAVE`, `S_RTC`), the snapshot's own
   fields and the three client messages are written out twice: in `protocol.rs` and by hand in
   `client/src/net/codec.ts` (485 lines). That is the last hand-held mirror. `terrain.ts` stays a
   copy on purpose (see `MIGRATION.md`, Phase 6).

Value: 1 and 2 matter most for more weapons and NPCs; 3 to 5 for more networked features. The order
below does 1 before 2 because the joined damage path is easiest to write with events to report into.
It does 3 and 4 before 5 because they change the messages that 5 generates.

## Rules for every step

Those of `MIGRATION.md`, unchanged:

- **Tests stay green.** `cargo test --workspace` and `cd client && npx vitest run` pass at every
  commit; tests that poke a changed API are rewritten, keeping what they assert.
- **Behaviour is kept.** Output that was in id order stays in id order; `world.rng` is drawn from in
  the same order.
- **No performance regression.** `full_room_tick_cost` (release, fixture city) before and after
  each phase; snapshot bytes (`replication/tests.rs`, 30 runners) before and after each wire change.
- **Clippy `-D warnings`, `vue-tsc` and oxlint add nothing.**
- **One commit per logical step**, `MIGRATION-II.md` updated in it.
- **Wire changes bump `PROTOCOL_VERSION`** (39 at the start), once per phase that changes it.

## Phase 1: events

**Goal.** A mechanic says what happened; whoever cares reads it. Damage code no longer builds wire
messages. Measure: no `world.hits`, `world.kills` or `world.noises` left. Adding a reaction to a death
touches no file under `mechanics/combat*`.

- [x] 1.1 Choose the queue. Either bevy's `Messages<T>` as a resource of the ECS world or a small
      `Events<T>` of our own on `World`. Both must keep send order and be cleared once a tick, after
      `Game::step` has read them. Measure an empty tick and the full room with each; take the
      cheaper unless the difference is noise.
- [x] 1.2 `Noise` as an event: `make_noise` sends it, `Bots` reads it. Smallest family first, as
      roosters were in Phase 2.
- [x] 1.3 `Damaged { victim, amount, lost, attacker, weapon, from, at, head, kind }` and
      `Died { victim, killer, weapon, style, yaw, power }` events, sent where `damage_hit` and
      `npcs.rs` push now. A `Feedback` step in `Game::step` turns them into `Hit` and `Kill` wire
      messages, summed and ordered as `hits` and `kills` are now. Kill credit (`kills`, `deaths`)
      moves to a reaction on `Died`; the bot's `hit_from` moves to a reaction on `Damaged`.
- [x] 1.4 Delete the three lists; rewrite the 19 test sites to read events.

## Phase 2: a player's life is the generic one

**Goal.** One hit test, one damage path, one death for everything that can be hurt. Measure:
`first_player_hit`, `body_of`, `damage`, `damage_from`, `damage_hit` and `Player::hp` are gone;
`first_living_hit` is a single scan.

- [ ] 2.1 `Health` on players, with `Player::hp` removed. The respawn countdown becomes `Respawn`
      (players respawn at a spawn point, not at `Home`: a `Respawn` with a `SpawnPoint` choice in
      place of `Home`, or a player-only reaction to the countdown finishing).
- [ ] 2.2 The hurtbox learns what players need:
      - a head zone (`head: Option<f32>`, height above the centre; roosters have none);
      - a body kept up to date each tick by posture: standing, seated in the open (from
        `seated_head`), or none while sealed;
      - the vehicle it sits in, for the glazed second look.
      Players get one, set by `Movement` and the seating code.
- [ ] 2.3 Lag compensation for every hurtbox. `World::position_at`/`head_at` become a `Trail`
      component (the last few ticks of the hurtbox's centre). The generic test takes `back` and
      reads it, so a lagging shooter hits a rooster where they saw it too. This changes rooster
      behaviour slightly, and the change is the point; say so in the log.
- [ ] 2.4 One damage path: `damage_entity` takes the attacker, weapon, source and headshot flag,
      applies teams, the multiplier and armour, and sends Phase 1's events. The player-only
      consequences become reactions to `Died`, guarded by `With<Player>`:
      - getting out of the seat;
      - `interrupt`;
      - `fire_cooldown`;
      - the death style and throw.
      `DropOnDeath` and `Respawn` stay generic.
- [ ] 2.5 Delete the player-only functions; `first_living_hit` becomes `first_hurtbox_hit`.
      Measure the full room: the hit scans now cover players and creatures in one pass.

## Phase 3: owner-only components (protocol v2.1)

**Goal.** What only its owner sees travels as components too, diffed against the baseline like
everything else. Measure: `SelfState`, `DriveState` and the `me`/`drive` snapshot fields are gone.
A new private fact is one component with an `owner` flag.

- [ ] 3.1 An `owner_only` marker in the replicated schema (`shared::replicated`): such a component
      goes only to the receiver who owns the entity. The owner is the player's own entity, or the
      vehicle they drive. The marker is generated to the client like the rest.
- [ ] 3.2 `Vitals { hp, progress }` and `Hotbar { slots }` on the player's entity, owner-only. Health
      may then simply be `Vitals`; decide when Phase 2's `Health` is in.
- [ ] 3.3 `Motion { v, w, steer, spin }` on a driven vehicle, owner-only to its driver.
      `VehicleDriver` reads it from the entity store.
- [ ] 3.4 Remove `me` and `drive` from `Snapshot` and the codec; bump `PROTOCOL_VERSION`. Measure
      bytes: the hotbar is now sent when it changes, not every snapshot, so they should fall.

`ack` and `ride` stay in the header: they are facts about the receiver's connection, not about an
entity.

## Phase 4: `Seated`

**Goal.** A rider is a stickman with a component, drawn by the stickman presenter. Measure: the
client no longer derives riders from the vehicle's seat list, and `replicate.rs` no longer filters
on `embodied()`.

- [ ] 4.1 `Seated { vehicle, seat }` replicated on a seated player's entity, with no `Place` (where
      they are follows from the vehicle and the kind's seat). Interest uses the vehicle's place.
- [ ] 4.2 The client's seated stickman is drawn from `Seated` by the stickman presenter, using
      `seatView`/`seat_head`. The vehicle presenter stops placing riders.
- [ ] 4.3 Drop the riders from the vehicle's replicated `Seats`, if nothing else reads them (the
      server's `Seats` stays authoritative). Bump `PROTOCOL_VERSION` with Phase 3's if they land
      together. Measure bytes against Phase 3's.

## Phase 5: generated messages

**Goal.** No message is written out by hand on the client. Measure: `codec.ts` holds framing,
the `decodeServer` dispatch and the input encoder's packing, and no field lists. A new message is
one Rust struct.

- [ ] 5.1 Generate a schema for each message struct in `protocol.rs` (field names and types, the
      order MessagePack arrays use), as `replicated.rs`'s tests generate `components.generated.ts`:
      `client/src/net/messages.generated.ts`, refreshed with `UPDATE_PARITY=1 cargo test -p shared`.
- [ ] 5.2 Decode the reliable messages (`S_KILL`, `S_HIT`, `S_SCORES`, `S_DESTROYED`, `S_DAMAGED`,
      `S_CARVED`, `S_PROFILES`, `S_NOTICE`, `S_LEAVE`) from the schema. A fixture of every message
      encoded by Rust, decoded by the old code and the new, must match before the old is deleted.
- [ ] 5.3 The same for `S_WELCOME`, the snapshot's fields and the client's `C_INPUT`, `C_PING` and
      `C_RTC`, under the same fixture. Measure decode time on a recorded snapshot stream; the hot
      path must not slow.
- [ ] 5.4 Not planned unless 5.3's measurement says so: decoding snapshots in Rust through wasm.

## After

Merge to `master` once all five phases are done, after a play session: shoot players and roosters
from a car and on foot, die and respawn, drive with a passenger. Then close `MIGRATION.md`'s open
boxes by pointing them here.

## Log

### 2026-10-23

- Merged `refactor/ecs-migration` into `master` (`5c755f3`, no fast-forward) after the user played
  Phases 5 and 6. Branched `refactor/ecs-migration-ii`; wrote this plan.

- **1.1, 1.2.** `crate::events` holds the event kinds, each a `bevy_ecs` `Messages` resource in the
  room's ECS world; `World::send` and `World::events` reach them, and `Game::step` moves every queue on
  once a tick (`events::update`), after the reliable messages are sent. `Noise` is the first: `make_noise`
  sends it, and `Bots` hears it at the start of the next tick through its own `MessageCursor`, so each
  noise is heard once, as `std::mem::take` gave it. `World::noises` is gone.
  - The queue was chosen by measurement (a throwaway release micro-benchmark, three rounds). Four sends
    and a read per tick cost 19 to 22 ns with a `Vec` and 36 to 45 ns with `Messages`. Reading and
    moving on an empty queue costs 19 to 21 ns. Against a 1.7 ms tick both are noise, so `Messages` was
    taken: it gives every reader its own cursor and keeps send order, which a reaction needs and a
    drained `Vec` does not.
  - `full_room_tick_cost`, release, three runs: mechanics 1.65 to 1.68 ms (baseline today 1.75 to 1.79),
    snapshots 601 to 620 µs (651 to 669), `Bots` 156 to 229 ns (170 to 188). Within the machine's noise.
    Server tests 271, unchanged; clippy clean.
- **1.3, 1.4.** `Damaged { victim, lost, attacker, weapon, from, at, head, killed, aboard }` is sent by
  every damage path (`combat::damage_hit`, `damage_vehicle`, `life::damage_entity`), and
  `Died { victim, killer, weapon, style, yaw, power }` by a player's death. `World::hits` and
  `World::kills` are gone.
  - `Mechanic::react` runs after every mechanic's tick and the scenario's, before the players are told.
    Two mechanics answer events through it. `Scores` (new, `mechanics/scores.rs`) counts kills and
    deaths from `Died`. `Bots` turns a hurt bot, or one aboard a hurt vehicle, towards `from`.
    A new reaction to a death is a `react`; nothing under `mechanics/combat*` changes.
  - `game::feedback` builds the wire messages from the events: `hits` (taken by the victim if a player
    and by whoever was aboard, dealt to the attacker if not themself or aboard; the kind from the
    victim's components) and `kills`. `Game::step` sums and sends them as before. Scores are counted
    for the whole tick before the `Kill`s are built, so each `Kill` takes back off any later kill of the
    same tick. Two kills in one tick still read 1 and then 2 (new test).
  - `life::damage_entity` takes the attacker, weapon and source, so roosters' markers come from the
    same events (Phase 2 joins the rest). `hit_mark` is private to it, and `combat::hit_at` is gone.
  - Two small changes, both to a creature's marker. A rooster's marker now shows the health it lost,
    not the damage asked for, so a 30-damage round on a rooster with 2 hp left reads 2, as a player's
    does. A round whose damage had fallen off to 0 no longer shows a marker of 0.
  - Tests: the 19 sites read the events (`World::happened`) or `feedback::hits`. Two tests that called
    a damage function directly and then read a score now check the `Died` event and step once.
  - `full_room_tick_cost`, release, three runs: mechanics 1.63 to 1.68 ms, snapshots 611 to 628 µs,
    `Combat` 29.5 to 31.1 µs, `Scores` 15 to 17 ns. Server tests 272 (one new); clippy clean.
