# Vehicle dealers

Some stores sell vehicles. A dealer is an OpenStreetMap store, those with the biggest car parks chosen first. The importer gives it a carport of the game's own against the store's wall that faces the car park, or against its front where it has no car park. A soldier standing at the carport opens the phone's **Dealer** app and pays for a vehicle from their delivery cash ([DELIVERY.md](DELIVERY.md)). The vehicle stands ready under the carport's roof, facing out onto the car park. A helicopter is set down on the car park in front of the carport instead.

There are about one or two dealers to a square kilometre in town, fewer in the country, and none where there are no stores. The home world (41 km², mostly forest and fields round Umeå) has 22; Holmön and Vindeln have one each.

## Buying

- Stand within `DEALER_REACH` (25 m) of a carport's mouth, on foot. The hint reads **E: buy a vehicle** when nothing else is in reach. **E** then brings the phone up on the Dealer app. **T**, **M** or the tabs work as always.
- A player joins with `STARTING_CASH` (1000), enough for a car or even a helicopter before their first delivery. Bots, civilians and the police have none.
- The app lists everything a dealer sells, `DEALER_KINDS`, with its price, `DEALER_PRICES`. The prices are set against what deliveries pay (about 20 to 110 each):

  | Price | Vehicles |
  |---|---|
  | 20–80 | snowracer, golf cart, EPA tractor, snowmobile |
  | 120–200 | the period saloons and estates, Transporter |
  | 250–450 | jeep, roadster, sports car, police car, ambulance, bus |
  | 400–700 | technical, Pak, mortar, Pbv |
  | 900–1000 | AMV, air ambulance, helicopter |
  | 1200–1500 | Strf 90, T-55, Strv |

- A price button is grey away from a dealer and when the cash falls short. Away from a dealer, the app says how far the nearest is. The map and the minimap mark each dealer with a white car on blue.
- A dealer sells no planes, no walkers (the walker, the hen and Tupp), no boats or submarine, and not the Titan. Drones, missiles and the ship's gun are not for sale either.
- Tapping a price sends `Phone { action: PHONE_ORDER, job: kind }` (protocol 103). When the server takes the order, it sends `Ordered` to everyone: the buyer's cash goes down on the scoreboard, the buyer is told that the vehicle is ready, and that carport's roller door rolls up for a few seconds.
- There is no limit for each player. An order is refused, and nothing is paid, when:
  - the buyer is dead, seated, on a lift, or not within reach of a dealer;
  - the kind is not sold, or the buyer's cash is short;
  - the world already has `MAX_VEHICLES`;
  - another vehicle still stands within 7 m of where the new one would go;
  - the new one would not fit there (`Physics::car_fits`).

  A carport holds one vehicle at a time, so drive the last one out first.
- A bought vehicle is like any other while it lasts, but it is `Bought`. Once it has burnt out, fallen out of the world, or stood stranded with an empty tank, it is gone for good. It is not put back in the carport as new, as a placed vehicle is at its home (`mechanics::vehicles`, `reclaim_stranded`).

## Selling

- Drive a vehicle forwards in under a carport and up to its roller door. The door rolls up as you come under the roof. When the vehicle's nose is within `DEALER_SELL_REACH` (1 m) of the door, heading for it within 45° (`shared::dealer::driven_in`), the dealer takes it. Backed in, nose out, it is not taken, so a vehicle just bought, which stands facing out, is never sold straight back.
- The dealer buys only kinds it sells (`DEALER_KINDS`). It pays the driver the price over `DEALER_RESALE_DIVISOR` (a sixth), times the share of its health left (`resale`). A car at half its health fetches a twelfth.
- Only from a player the police are not after: a driver with wanted stars is told "The dealer won't buy from someone the police are after", once each time they drive in, and keeps the vehicle. Lose the stars first, at a car wash ([FUEL.md](FUEL.md)) or by hiding.
- Only a person at the wheel sells. A bot driving in sells nothing.
- Everyone aboard is set down at once. Then the vehicle goes:
  - one placed in the world (by the editor or the scenario) goes back to its home, as new, so the world keeps its vehicles;
  - one bought at a dealer (`Bought`) is gone for good;
  - one taken from the traffic (`Abandoned`) or from a bus line (`Strayed`) is taken away, as it would be once left empty anyway. The traffic and the bus lines already replaced it when it was taken: a fresh car starts somewhere nobody is looking, and the line gets a new bus after `BUS_RETURN`.
- The server sends `Sold` to everyone (protocol 104): the seller's cash goes up on the scoreboard, they are told what they got, and that carport's door rolls up.
- The `Dealers` mechanic (`crates/server/src/mechanics/dealers.rs`) looks at every vehicle each tick.

## The carport

- Every carport has the same layout in its own frame (`shared::dealer`, `Dealer { at, yaw, base }` in the manifest):
  - X runs across its mouth, and +Z points out of the mouth onto the car park.
  - It fills `DEALER_PORT` (8 m clear across, 14 m deep) from the store's wall to its mouth, with `DEALER_PORT_HEIGHT` (5 m) clear under its roof. That is room for the bus and a tank with its gun.
- **What is solid** (`shared::dealer::structure`): a concrete floor, three steel posts down each side, and a flat roof. The server and the client's physics stand colliders of these from the start, as they do a fuel station's (`sim`'s `add_dealers`). The client draws the same parts through the WASM's `dealerParts`, so what is seen is what is hit.
- **What is only drawn** (`client/src/render/dealers.ts`):
  - a blue fascia round the roof;
  - a sign on the roof's front edge with the dealer's name, "NORD BIL", an invented chain like the fuel stations' NORD;
  - lamps under the roof, lit by night;
  - white lines down the floor;
  - a ribbed roller door in the store's wall, over a dark doorway, which rolls up when a vehicle is bought there.

## Where the dealers come from

`crates/osm-import/src/dealer.rs` chooses them after the world is assembled:

1. **Car parks.** Every surface `amenity=parking` of OpenStreetMap: not underground, multi-storey or on a roof (`assemble::is_surface_parking`, the same test that paves them as asphalt). Each is its outer ring.
2. **Stores.** Every building of the finished tiles whose kind is `BUILDING_SHOP`.
3. **A site for each store** (`site`):
   - A store's score is the area of car park within 30 m of it: across the service road that often runs between a store and its car park.
   - The carport faces the biggest of those car parks when they come to at least `DEALER_MIN_PARKING` (800 m²). A store with less faces the road it stands by (`Building::front`). A store by neither takes its longest wall.
   - It needs a wall facing that way, within 60°, at least as long as the carport is wide.
   - The carport's back is set against that wall, opposite the car park's middle (or the middle of the wall), and kept back from the wall's ends.
   - The site is refused when:
     - the carport, or 8 m of apron in front of its mouth, covers more than 1 m² of another building, or more than a sliver of its own store;
     - it stands on a main road, or on any road 6 m wide or more (narrower service roads are a car park's own aisles and drives, which a carport may stand over);
     - it stands within 35 m of a fuel station;
     - a corner, its middle or its apron is in the water.
   - The next best wall is tried before giving up.
   - The import prints how many stores there were, how many could have a carport, and why the others could not. In the home world, 111 of 191 could: 17 had no long enough wall, 36 had a building in the way and 27 stood on a main road.
4. **Choosing** (`choose`). The sites with the most car park come first, so a store with a big car park wins over its neighbours, and one with none is only chosen where nothing better is near. No dealer stands within `DEALER_APART` (700 m) of one already chosen, which leaves about one or two to a square kilometre where stores are many.
5. **Laying** (`lay`):
   - The ground under each carport is levelled to the middle height of the ground there and eased back into the slope round it, as a fuel station's lot is (`fuel::level`). It is also paved (`fuel::pave`).
   - The trees on the carport and its apron are taken out.
   - The dealers go into `Manifest::dealers`.

No building is taken out of the tiles. A store whose carport would need a neighbour gone is passed over, so nothing is renumbered after the tiles are built. The import also prints how many car parks it found and how many dealers it laid, with the dealers to a square kilometre over the whole world. The synthetic city has one dealer, a store and its car park in block `SYNTHETIC_DEALER`, so the fixture world and SOLO have one.

## Code

| Where | What |
|---|---|
| `crates/shared/src/consts.rs` | `DEALER_*`: the carport's size, the reach, the spacing, the least parking, the helicopter's spot, the catalogue and its prices; `PHONE_ORDER` |
| `crates/shared/src/dealer.rs` | the carport's frame and solid parts, where a bought vehicle stands (`port`, `heli_spot`), `at_dealer`, `price`, `driven_in`, `resale` |
| `crates/shared/src/world.rs` | `Dealer` and `Manifest::dealers` (world version 44) |
| `crates/shared/src/protocol.rs` | `Ordered` (`S_ORDERED`, protocol 103), `Sold` (`S_SOLD`, protocol 104) |
| `crates/sim/src/lib.rs` | the carports' colliders (`dealer_colliders`, `add_dealers`) |
| `crates/server/src/mechanics/dealers.rs` | `order`: the checks, the payment and the new vehicle; `Dealers`: buying back what is driven in |
| `crates/osm-import/src/dealer.rs` | choosing the stores and laying the carports |
| `client/src/render/dealers.ts` | the carports, drawn, and their doors |
| `client/src/phone/dealerapp.ts` | the phone's Dealer app |
| `client/src/prompt.ts`, `client/src/game.ts` | the hint and E, the map marks, `Ordered` |

## Tests

- `shared::dealer` tests:
  - a bought vehicle stands under the roof, facing out;
  - a dealer is reached from near its mouth;
  - the roof leaves its clear height;
  - a vehicle is driven in when its nose reaches the door heading for it;
  - a dealer buys back at a sixth of the price, less for damage;
  - every kind sold is a vehicle a dealer may sell, priced, and fits the carport.
- `server` `mechanics::dealers::tests`:
  - an order takes the price and stands the vehicle in the carport;
  - nothing happens away from a dealer or without the cash;
  - no plane or Titan is sold;
  - a second order waits while the carport is taken;
  - a helicopter is set down on the lot;
  - a bought vehicle that burns out is gone, not put back in the carport;
  - a placed car driven in through the door is sold for a sixth and goes home as new;
  - a bought car sold back is gone, and a hurt one fetches less;
  - a wanted driver, or a car backed in, is not bought from.
- `osm-import` `dealer::tests`:
  - a store by a big car park gets a carport against the wall facing it;
  - a store with no car park, or a small one, faces its road, and a far car park is not counted;
  - a carport on a neighbour, on a main road, by a fuel station or in the water is not built;
  - a store too small for the carport gets none;
  - of two stores closer than the spacing, the one with more parking wins, and one with none far off is chosen too;
  - a house is never a dealer;
  - the synthetic city has one.
- `phonescreen.spec.ts`: the Dealer tab, and buying only at a dealer and only what the cash covers.
