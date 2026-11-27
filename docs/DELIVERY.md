# Deliveries

A game mode for any world with shops and houses: a phone full of food orders. Take one, fetch the bag at the restaurant's door, and run, drive or fly it to the customer's house before it goes cold. Whoever hands it in is paid, and that need not be who took the order: kill a courier, grab their bag and deliver it yourself.

## Turning it on

Put a top-level `deliveries` in the world's `scenario.json` (see [SCENARIOS.md](SCENARIOS.md)):

```json
{ "deliveries": {} }
```

`{ "jobs": 10 }` sets how many orders the board holds at once (6 without it). It works on its own or alongside the backpack, squads and triggers. A world without a shop, or with fewer than two houses, stops the server at start-up with the counts named.

## Rules

- **The phone.** The orders are the Deliveries app on the phone ([PHONE.md](PHONE.md)): T brings it up there (M on the map), and the tabs switch between the two. It frees the mouse to tap it, but the keys still move you, so you can walk while you choose.
- **The board.** Every order is a parcel from a shop (`BUILDING_SHOP`: restaurants, cafés, shops) to a house (a plain house, villa, block of flats, farmhouse or cabin) 150–1500 m away, picked at random. The phone lists them nearest restaurant first, with how far the restaurant is, how long the trip is, the pay and the time allowed. The world names no shops or houses, so the client makes up a restaurant and a household for each order from its number, the same for everyone.
- **Taking one.** Tap an order's price to take it off the board. A new one takes its place. You can have one order at a time: one to fetch or a bag on your back. Until you fetch it you can give it back (Cancel), and it goes back on the board.
- **Fetching.** Walk to the restaurant's door (within 3 m, on foot) and the bag goes on your back: a teal cube with a white band. The clock starts.
- **Delivering.** Walk to the house's door with it, on foot. You are paid `20 kr + 60 kr a kilometre` of the shop-to-house distance. That is the full pay within the order's par time, 60 s plus the distance at 5 m/s. After that the pay falls steadily, down to a quarter of it once as long again has gone by, and stays there.
- **Robbing.** A courier who dies drops the bag where they fall, as the backpack falls. Anyone on foot without a bag walks onto it to take it, and can deliver it for the pay. The courier's phone shows the order as lost, pointing at where the bag lies, or at the customer's house once someone has picked it up, which is where the robber is heading. A courier who leaves the game drops their bag the same way, and an order they had not fetched goes back on the board.
- **Spoiling.** A bag left lying for 2 minutes spoils, and the order is over.
- **Finding the way.** The minimap and the phone's map (M) show a teal pin at the restaurant, the house or the lost bag. Off the map, a pip on its edge points the way. A line under the objective says where to go, how far it is, the pay now and the clock.
- **Cash.** The scoreboard (Tab) gets a **Cash** column. Every delivery tells everyone who delivered it and what they earned.
- **Bots** do not deliver, but they shoot couriers like anyone else.

## Tuning

Every number is a `DELIVERY_*` constant in `crates/shared/src/consts.rs` (run `npm run gen:consts` in `client` after changing one): `DELIVERY_BASE` (20) and `DELIVERY_PER_KM` (60), `DELIVERY_PAR_GRACE` (60 s) and `DELIVERY_PAR_SPEED` (5 m/s), `DELIVERY_FLOOR` (0.25), `DELIVERY_MIN_DISTANCE` and `DELIVERY_MAX_DISTANCE` (150 and 1500 m), `DELIVERY_REACH` and `DELIVERY_REACH_Y` (3 and 2.5 m), `DELIVERY_SPOIL_SECONDS` (120) and `DELIVERY_JOBS` (6).

## How it works

- **Server.** The `Deliveries` mechanic (`crates/server/src/mechanics/deliveries.rs`) keeps `World::deliveries`, which is `None` without the mode.
  - `DeliveryState::from_map` collects every shop's and house's door at start-up: the middle of the first door (not a gate) among the building's openings (`windows::openings_of`, as `BuildingLook::windows`), on its sill.
  - Each `Job` has its two doors, its pay and par time, its courier and where its parcel is (`Whereabouts`: at the shop, carried, or lying).
  - Carried, a parcel is a `Parcel` component on the carrier. Lying, it is an entity with `LyingParcel` and `Transform`, replicated as `kind` 36 (`KIND_PARCEL`).
  - Where a parcel falls and who walks onto it is shared with the backpack (`mechanics/carry.rs`).
  - Taps on the phone come as `Command::Phone` and go to `deliveries::act`.
- **Wire.**
  - `phone` (client tag 5) is a tap: `PHONE_ACCEPT` or `PHONE_CANCEL` and the job. It goes down the client's signaling stream, so it is never lost.
  - `jobs` (20) is one player's phone: the board and their own jobs (`JOB_FETCH`, `JOB_CARRY` or `JOB_LOST`, where to go, the pay now and the clock). It is sent to everyone when the board changes, to a player when their own jobs change, once a second while they have one, and to a joiner.
  - `delivered` (21) goes to everyone: who handed in which job, whose it was, the pay and their cash in all. A player's cash starts at `STARTING_CASH` (1000), for the dealers ([DEALERS.md](DEALERS.md)). It is sent before the phones, so a job gone from a phone is known to be delivered, not spoiled.
  - `Score` carries `cash`, and the `carry` component's `CARRY_PARCEL` (16) bit puts the bag on a soldier's back. Protocol 92.
- **Client.**
  - `DeliveryTracker` (`client/src/deliveries.ts`) keeps the phone, names the shops and households, and says what changed.
  - The Deliveries app (`phone/jobsapp.ts`) is the board on the phone ([PHONE.md](PHONE.md)). Its taps go to `game.phone`.
  - `hud/map.ts` draws the pin (`MapMarks.target`), and `hud/panels.ts` draws the job's line (`deliveryLine`) and the Cash column.
  - `render/parcel.ts` builds the bag, worn (`Character.parcel`) and lying (`entities/presenters/parcels.ts`, with the backpack's bob and ring).
- **Tests.** `mechanics/deliveries/tests.rs` covers the board, taking, cancelling and fetching an order, the pay before and after par, dropping on death and on leaving, a robber delivering, one bag at a time, spoiling, and a world without shops. On the client, `deliveries.spec.ts` covers the tracker, `phonescreen.spec.ts` taking and giving back an order on the phone, and `scoreboard.spec.ts`, `minimap.spec.ts` and `codec.spec.ts` the cash, the pin and the wire.

## Later

Real street addresses from OpenStreetMap (`addr:street`, `addr:housenumber`) and restaurant names (`name`), a fence for stolen bags, a share for whoever escorts a courier, bots that deliver, and achievements.
