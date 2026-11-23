# Traffic: a thousand cars going about their business

A world can have traffic: cars that drive its roads on errands of their own, keep their distance, slow for bends and give way at junctions. `home` has two for every kilometre of lane they drive, about 1200. They are ordinary cars to look at, to shoot at, to be run over by and to take.

Nobody can see a thousand cars at once, and that is what makes it possible. A vehicle as the game knows it is a rigid body on sprung wheels that the physics steps every tick, about 0.025 ms each when it moves (BUSES.md), so a thousand of them would be 25 ms of a 16.7 ms tick. On `home` 1000 cars over 42 km² puts a few dozen within sight of any one player. Only those need to be vehicles, and only the ones beside a player need their physics.

## In play

- **Cars** are the period cars of 1980 to 1995 ([VEHICLES-XXIII.md](VEHICLES-XXIII.md)): Saabs, Volvos, Opels, Fords and Golfs twice as often as Broncos, Mantas, Transporter vans and EPA traktors. An EPA does 30 km/h and no more, whatever the road, so it is what the others queue behind and pass. They drive on the right, at up to 22 m/s on a big road and 10 m/s on a small one, and slower through bends and turns. Each has a pace of its own, from three quarters of its road's speed to 15 % over it.
- **At a junction** a car waits its turn. A junction belongs to one road into it at a time: the cars on that road follow each other across for up to 15 seconds, then the other roads get theirs. A car on a small road gives way to one arriving on a bigger road.
- **They stop for you.** A car brakes for someone on foot in its path and for a vehicle ahead of it, on the path it will drive and not just straight ahead of its nose. It waits for a person as long as they stand there.
- **They go round what is left in the road.** A vehicle standing in a car's lane (a wreck, a car you parked) is gone round, slowly, on the other side of the road once nobody is coming, or in the next lane where the road has one. Cars coming the other way wait 20 m off for one that is out.
- **They overtake.** A car that catches a slower one pulls out and passes where the road is straight, the line down its middle is not solid, and nobody is coming for as far as the pass takes. One that misjudges it drops back and pulls in behind again.
- **They keep off a broken bridge.** When a deck is blown through, its road is shut within a few seconds, and so is whatever road only led to it. Cars already heading for it drive to the end of their road and stand there; those on the broken span are left to the physics.
- **Someone drives them.** A car near enough to see into has its driver at the wheel.
- **They can be taken.** Press E beside one that stands or crawls (under 3 m/s) and its driver is dragged out, and runs. With you at the wheel it is yours, and it is no longer traffic. A car that is driving is not boarded.
- **Aim a gun at one** (right mouse, down the sights) from within 30 m and keep it there for 0.4 seconds, and its driver does one of three things. A gun that is only in your hand is nothing to them, whichever way it points. Four in ten stop, get out with their hands up for two seconds and run, leaving you the car. Four in ten flee. Two in ten drive at you for up to 4 seconds if you are ahead of them and within 40 m, and then flee.
- **A driver who flees** goes flat out on the straights and faster than traffic through the bends for 20 seconds, waits at no junction, sits a metre off the car ahead, and passes whatever is slower wherever they are: across a solid line, into the face of whoever is coming, round a queue. One that is boxed in for 3 seconds gets out and runs.
- **Shoot the car** and half the drivers lose their heads: flat out, braking for nothing, the wheel thrown from side to side, until the car hits something or 12 seconds are up. Then they get out and run, if the crash left them a car to get out of. The other half do as their nerve has them. A driver who is hit themselves always panics.
- **A blast** within 150 m makes drivers hurry for 12 seconds: 1.4 times their pace, closer to the car ahead, and readier to pass. One within 40 m they take as meant for them: they flee, panic, or stop and run for it.
- **Gunfire** within 80 m makes them hurry for 8 seconds. A round that goes by within 4 m of the car makes a timid driver stand on the brake and get down for two to four seconds, and then drive off; the others drive off at once.
- **The driver can be shot at the wheel**, as a player in that seat can: over the side of a Bronco, through the glass of any other car. Wounded, they take it as a threat. Shot dead in a car that is going, they stay slumped at the wheel and it runs on, a little off straight, the foot still down half the time, until it stops against something or has run 8 seconds; then the body falls out beside the door.
- **A driver on foot** runs from whatever frightened them, round what is in their way, and is gone once no player is within 150 m. They can be shot, and run over.
- **A badly damaged car leaves the road.** Under half its health, or with a wheel gone, it pulls onto the verge on its right, and its driver gets out and runs. A scratched one drives on.
- **The roads clear themselves.** A car that has stood 45 seconds with nobody watching starts afresh somewhere else; one that is watched is left by its driver after two minutes. Cars that are nobody's any more (given up, pulled over, taken by a player and left, driverless) are taken away, sooner when they block the road and sooner still when nobody is looking: see [Healing](#healing).
- **They can be wrecked.** A wreck burns where it lies and is taken away after 8 seconds, its driver with it.
- **The roads stay as busy.** For every car taken or wrecked, another starts somewhere nobody is looking. A car that drives out of the world by a road that leaves it comes back in by another.

Not there yet: passengers, lamps at night, police and ambulances, parked cars, traffic lights, and a way round a shut bridge for a car with somewhere to be: traffic wanders, so it only stops choosing that road.

## Turning it on

| Where | What |
|---|---|
| `worlds/<name>/scenario.json` | `"traffic": { "per_km": 2 }` |
| Server | `RAGNAROK_TRAFFIC=n` puts `n` cars on each kilometre of driven lane whatever the scenario says |
| `World::traffic` | How many cars the room wants for each kilometre of lane the traffic drives. `Traffic` does nothing while it is 0. At most `TRAFFIC_MAX` (4000) cars |

The number follows the network: a road that leads nowhere is not driven and counts for nothing, and when a bridge is shut the cars that its roads accounted for are taken away.

Nothing in the world's files changes, and the client is sent vehicles and their drivers like any others.

## The three levels (`crates/server/src/mechanics/traffic.rs`)

| Level | When | What it is |
|---|---|---|
| On rails, unseen | No player within 320 m | A row in a table: the way it is on, how far along, its speed, the way it takes next. Moved once in four ticks, by four ticks' worth |
| On rails, shown | A player within 320 m (out again at 350 m) | A vehicle like any other, whose body is put where its row says each tick (`Physics::set_car_rails`, `place_car`). The physics works out no wheels, springs or engine for it |
| Physical | A player within 45 m, or it is hurt, boarded, held up for 8 s, or its driver has been threatened | Its body is let go at the speed it had. `Traffic` steers and throttles it along its lane (`Autopilot`), and whatever hits it moves it |

- 320 m is past the 300 m a player is told of vehicles at (`INTEREST_RADIUS`), so a car is already driving when it comes into view, and 350 m is past the 340 m a player keeps one they were told of.
- At most 384 cars are shown at once (`SHOWN_MAX`), within the 512 vehicles a world may have, and at most 96 of them are physical (`PHYSICAL_MAX`). Past either, cars stay as they are.
- A physical car goes back on rails when no player is within 70 m, its driver is calm and it drives its lane as the rails would have it: within 0.8 m and 0.12 rad, upright and sound (half its health or more, and all its wheels). Otherwise it stays physical until it is out of sight.
- A physical car that leaves the road by 20 m, lands on its roof or sticks three times is abandoned where it is if a player is within 60 m, and else taken away.
- A physical car's body is wherever its driver has got it, which lags what the traffic asked of it. So it looks ahead along where its body is as well as along where it is steering to (`Car::lateral`).
- "A player" is a person: bots, zombies and bus drivers do not make traffic real.

A car on rails rides at the height its springs would hold it (`Physics::car_ride_height`), so letting it go neither drops it nor makes it hop; a test in `crates/sim` holds that to 5 cm.

## The road network (`traffic/graph.rs`)

The world has its roads as centre lines cut at the tiles' edges, with nothing to say which meet. The server joins them into a graph once, the first tick traffic is wanted (23 ms on `home`):

- **A node** wherever a road ends or two roads share a point, to the metre, so the halves of a road cut at a tile's edge meet.
- **Levels.** Roads meet only where their surfaces are within 1.5 m of each other, and a point in the middle of a stretch a bridge carries is on a level of its own. So a road through an underpass does not meet the road on the bridge over it, even where one ends right under the other. `home` has 12 such points.
- **An edge** for each stretch between two nodes: its centre line with a point every 6 m at most, the height of the road's surface at each (the deck's on a bridge, found from below: see "Broken bridges"), its lanes each way and its speed.
- **A way** is an edge driven one way. Only the ways that lead somewhere are driven: one with a lane its way, that traffic can come into and get out of again by another road. A cul-de-sac goes, and whatever only led to it.
- **A portal** is the end of a road within 60 m of the world's edge. A car that arrives at one is gone, and comes in again at another.
- **A turn** from one way into the next is a curve from lane to lane over the last 7 m of one and the first 7 m of the other, taken at the speed its sharpness allows.

On `home`: 6997 nodes, 7983 edges, 420 km of road, and 10 812 of 15 966 ways driven, 605 km of them. The 90 km driven neither way is cul-de-sacs and what only leads to them.

What the world's roads do not say, and what is made of it:

- **No speed limits.** A big road with two lanes a way is driven at 22 m/s, another big road at 16, a small road at 10 and a single lane at 7.
- **Which way a road runs** is `Road::one_way` (world version 41): a road of a single lane is written the same whether it is one-way or not, and 160 of `home`'s 420 km is single lane. A single lane that is not one-way is driven both ways, each car keeping to its side of it.
- **No lanes on a bridge.** A bridge's own road is taken as one lane each way, and the wrong way over a divided road's bridge is found to lead nowhere and dropped.

## Driving (`limit`)

One rule gives how fast a car may go, on rails or physical: the least of its road's speed, the speed the bends ahead allow, the speed that keeps 3 m and a second's travel behind the car in front, the speed that stops short of what it saw in its way, the speed of the turn ahead, and the speed that stops 9 m short of a junction that is not yet its road's. A car on rails moves by that; a physical car's throttle and brake aim for it, and it steers at a point on its lane 5 to 14 m ahead.

A shown car looks ahead every fourth tick (`sight`): along the path it will drive, every 2 m for as far as it needs to stop in (up to 90 m), for a vehicle that is not on rails or someone on foot. Cars on rails are not looked for, as the network already keeps them apart. A car that is unseen looks for nothing: there is nothing there but other rows.

A junction is not handed to another road while the last road's cars are still on it, for up to 6 seconds after their time is up, so cars do not cross through each other.

Three things keep the roads from filling with cars that wait for each other:

- A car that has waited 5 seconds at a junction for room on the road beyond goes anyway. Round a small block of short roads each car would else wait for the one ahead for good.
- A car does not turn straight back to the node it came from by a road beside its own.
- A car that has stood for a minute where nobody sees it starts afresh elsewhere.

## Out of the lane (`decide`)

A car has a place across the road as well as along it: `shift`, metres left of its lane, moving towards `aside`, which is 0 or one lane's width (2.6 to 3.5 m). Out is the next lane its own way where its road has two, and else the other side of the road.

| | Passing a slower car | Going round a standing vehicle |
|---|---|---|
| When | The car ahead is 2.5 m/s slower than this one would drive (its pace, and no more than its kind goes), within 12 m and a metre more for every m/s, rolling, with 25 m clear ahead of it | A vehicle that is not traffic stands in its path within 25 m |
| Where | A straight stretch, no solid centre line, done 45 m or more short of the next junction | Anywhere the way round is clear and on the asphalt, done 12 m short of the next junction |
| Clear of oncoming | For the whole pass and what the fastest car could cover meanwhile, plus 40 m, counting a car that could come onto the road at the next junction | Nobody so near that they could not stop 20 m short of it: whoever comes waits |
| Speed | Its own pace | 4 m/s |
| Called off | Someone is coming who would be there before it is past and back in: it drops back behind the car and pulls in. Or the junction is near | Only before its nose is up to the thing: after that it goes on |

- "The next junction" is where a third road meets this one. A road is cut into stretches wherever anything touches it and at every tile's edge (53 m on average in `home`), and a car that is out stays out from one stretch of the same road onto the next (`road_ahead`). Held to one stretch, as it first was, a car in town never had room to pull out at all.
- A car that is out passing does not brake for a car coming the other way: it has allowed for it. The one coming brakes only if they are about to meet all the same. Both wait for a car that is out going round a standing vehicle.
- The length of a pass is worked out from how the passing car gathers speed (`gaining`), for starting it and for calling it off alike, so one is not called off the moment it starts.

- A car that stands waiting in a queue is not gone round: only a vehicle that is not traffic counts as standing.
- A physical car stops 14 m short of a standing vehicle, for room to steer out. If it eases up to it and still has not cleared it, it backs up a second and tries again.
- Every car finds the car ahead of it by where the cars on its lane are, not by the order they came onto it, since passing changes that.

## Drivers (`traffic/drivers.rs`)

A driver is a light entity, as a rooster is: a kind (`KIND_CIVILIAN`), a place and health, with no body in the physics, no input and no profile. That is why there can be a few hundred of them.

- **One for each shown car within 150 m of a player** (gone again beyond 180 m). Nobody sees into a car from further.
- **At the wheel** the driver is sent as a rider of seat 0 of their car, and the client draws them as it draws any rider. The vehicle's own seats stay empty on the server, so the autopilot goes on driving, and a player who boards takes seat 0. `Traffic` sees a player there, gives the car up (`abandon`) and puts the driver out beside the door (`put_out`).
- **Threatened** (`threats`, every fourth tick): a player on foot aims a gun (`BUTTON_ALT`) whose line of fire meets the car's hull within 30 m with nothing between them, for 0.4 seconds; or the car has less health than when its driver last looked.
- **Nerve** is rolled from the traffic's own dice when a row starts afresh: `GivesUp`, `Flees` or `Rams`, and with it whether the driver is `rash`. **Mood** is what the driver is about now, and `limit`, `decide` and `drive` read it:

| Mood | What the car does |
|---|---|
| `GivingUp`, `Ducking` | Held to 0 m/s |
| `Hurrying` | 1.4 times its pace, half the headway, passes with half the clearances. Stays on rails |
| `Fleeing` | Top speed, 1 m behind the car ahead, no junction waited at, passes with no checks at all |
| `Ramming` | Steers at the player instead of along its lane, with nothing in its way counted |
| `Panicking` | Throttle flat; steers for a point 2 to 5 m to one side of its lane, thrown again every 0.4 to 1.2 s, with 1.8 times the lock; no rule about leaving the road applies |
| `Runaway` | The driver is dead: a fixed wheel and throttle until it stops |
| `PullingOver` | For the verge at 4 m/s (see Healing) |

- **Hearing** (`hear`): `Traffic` reads the world's `Noise` events, which blasts and shots already make for bots and deer (`bots::listening` now counts a world with traffic). A noise with no line and a blast's reach is a blast; one with a line is a shot, and how near the car its round goes is the distance from the car to that line.
- **What counts as a hit on the car** is damage with someone behind it (`HurtBy`): a round, a blast, a player's car driven into it. A knock from another car of the traffic is only a knock.
- **Crashes are the game's own.** A panicking car is an ordinary physical car at full throttle: what it hits, what that costs it and whether it goes up are `vehicles::crash`'s business. A car on rails cannot be moved, so one that finds a vehicle within a metre ahead in its path is let off its rails first (`STRUCK_WITHIN`).
- **On foot** (`afoot`) a driver stands with their hands up if they gave the car up (`STANCE_SURRENDER`), then runs at 5 m/s straight away from the threat, turning aside while a ray 2.5 m ahead meets something.
- **They can be hit wherever they are.** A driver has a soldier's hurtbox. On foot it stands on their feet. At the wheel `life::anchor` hangs it from seat 0's head point of their car and names the car, which is all `life::body` needs to make the short capsule of a rider of it; so the hull stops what it stops for a player there, and a round through an EPA's glass goes on to the driver. `run_down_creatures` leaves a seated driver out: they are in a car, not under one.
- **Hurt at the wheel**, a driver takes whoever did it (`HurtBy`) as the threat. Dead, `threats` gives the car up, and `put_out` lays the body beside the door, where it lies 10 seconds.

## Healing

Left alone, the roads near a player silt up: a car given up at gunpoint stands in its lane, the cars behind go round it or queue, one of them is shoved off its line, and so on. Three rules undo that.

**Watched** means a player is within 150 m and the spot is within 60° of where they look, or within 40 m of them whichever way they face (`watched`). No ray is cast: a car behind a house counts as watched, which only makes it stay longer.

**A badly damaged car pulls over** (`Mood::PullingOver`). A car that is not `sound` steers for the verge at 4 m/s, its side half a metre past the road's edge, stops there, and is given up. If it has not got there in 15 seconds it is given up where it is. A sound car goes back on rails; before, any scratch kept a car physical for good.

**A car that stands too long gives up waiting** (`Car::stood`, counted for physical cars too):

| Car | Stood | What happens |
|---|---|---|
| Unseen | 60 s | Starts afresh elsewhere |
| Shown, not watched | 45 s | Starts afresh elsewhere, vehicle and driver gone |
| Shown, watched | 120 s | Its driver gets out and runs, and the car is nobody's |

A junction's turn is 15 seconds, so an ordinary wait reaches none of these.

**A car that is nobody's is taken away** (`Abandoned`). A player in it has it, for as long as they sit there. A player within 15 m of one that is in nobody's way stops its clock: it may be theirs.

| Where it stands | Not watched | Watched |
|---|---|---|
| In the traffic's way | 20 s | 90 s |
| Anywhere else | 45 s | 5 min |
| Wrecked | 8 s | 8 s |

- It is in the traffic's way for 10 seconds after a car of the traffic last found it standing in its path (`sight_along` says which vehicle that was), whether that car then went round it or not.
- At most 24 stand at once (`ABANDONED_MAX`). Past that the ones empty longest go, watched or not.
- So a car can vanish while someone looks at it, after a minute and a half in the road. That is the price of a road that clears with a player standing by it.
- Vehicles that were never traffic (the world's own, a player's) are not touched: a road blocked with those stays blocked, and the cars that queue at it come and go by the standing rule.

## Broken bridges (`inspect`)

When the network is made, the height of each bridge's roadway is recorded every 2 m along its road. One bridge is looked at each tick, so all 342 of `home`'s in under 6 seconds. Where the roadway is a metre below what it was, the road is shut (`Graph::shut`) and the driven ways are found again, which takes the roads that only led to the bridge with it.

The roadway is found from below: up from the ground to the deck's underside, then down onto its top from 3 m over that. Found from the sky, a stay cable, a pylon or a tree over the road would be found first.

`a_real_bridge_cut_across_is_shut` cuts the busiest bridge of a real world with shells and prints what the traffic made of it. On `home`: one road shut, six ways fewer driven, no car left on it.

## What it costs

`full_room_tick_cost` takes `TRAFFIC=n`, and `ON_ROADS=1` to stand the 64 players beside roads all over the world:

```sh
RAGNAROK_WORLD=$PWD/worlds/home/world.msgpack TRAFFIC=2 SPREAD=1 ON_ROADS=1 cargo test --release -p server -- --ignored --nocapture full_room
WORLD=$PWD/worlds/home/world.msgpack MINUTES=30 cargo test --release -p server -- --ignored --nocapture traffic_on_a_real_world
```

On `home` with 1200 cars (`TRAFFIC=2`), 64 players each beside a road of their own:

| Room | Shown | Physical | `Traffic` | `Vehicles` | A snapshot | Whole tick |
|---|---|---|---|---|---|---|
| No traffic | | | | 0.30 ms | 64 bytes | 4.6 ms |
| Traffic as it was first built | 384 | 55 | 0.44 ms | 2.44 ms | 187 bytes | 7.4 ms |
| Traffic now, with drivers | 377 | 34 | 0.26 ms | 1.36 ms | 220 bytes | 6.1 ms |

With nobody connected the whole room takes 0.3 ms a tick. So traffic costs up to 1.5 ms a tick in the worst room, where it cost 2.8 ms. What brought it down, each measured by itself:

| Change | Saved |
|---|---|
| A car on rails is not looked at for trees and fences to drive through (`plow_trees` asked the physics three times a tick for every moving vehicle) | 1.0 ms |
| Physical within 45 m, not 80 m, and back on rails beyond 70 m, not 120 m | about 0.1 ms: fewer wheels to work out |
| An unseen car is moved once in four ticks; a car looks for who it meets only on a road where some car is out of its lane; a bridge is looked at every fourth tick | 0.17 ms |

What is left is the physics: about half of `Vehicles` is the broad phase finding what each moved body is near in a world of a quarter of a million trees and buildings, and a quarter the wheels of the physical cars. Setting the far cars down as bodies that stand, moved only in the ticks a snapshot is taken after, measured no faster and was taken out again.

The worst tick in that room, about 42 ms, is `Combat`'s (37 ms of it) and is there without traffic too.

With nobody connected the cars average about 8 m/s (their paces average under their roads' speeds, and a sixth of them are EPA traktors), with 30 to 50 of the 1200 standing at any time, nearly all of them waiting their turn at a junction. There are about 14 passes a minute, and three to six cars out passing at any moment.

`network_around_a_place` prints the network round a point, for a junction that does not work:

```sh
WORLD=$PWD/worlds/home/world.msgpack AT=-3378,-841 REACH=40 cargo test --release -p server -- --ignored --nocapture network_around
```

## Tests

- `traffic/graph.rs`: a side road meets a through road at a node, and a cul-de-sac is not driven; a one-way ring is driven one way round, slowly at its corners; a bridge is on its deck and does not meet the road under it, nor one that ends under it; a bridge that is shut takes the roads that only led to it; a turn is one smooth curve.
- `traffic/tests.rs`: forty cars for ten minutes keep to live roads, apart, and moving; a car is shown near a player, let go beside them and hidden when they leave; a physical car drives its lane and goes back on rails; a car stops for someone standing in the road; a car a player takes is theirs, and another takes its place; a slow car is passed on the open road, and never into the face of another; cars go round a vehicle standing in their lane without touching it, on the open road and in a town street between two junctions; a road whose bridge is down is shut, the traffic keeps off it, and there are as many cars fewer as there is less road; a car near a player has a driver at its wheel and a far one has none; a player who takes a car has it, and its driver is out on the road running; a moving car is not boarded; a driver with a gun on them gives up, flees or drives at the gunman, as their nerve is; a driver at the wheel is hit by a round aimed at them and panics; one shot dead at speed is carried on by the car until it stops, and then falls out; a blast in earshot makes drivers hurry, and one beside a car frightens its driver; gunfire makes them hurry, and a round past the car makes a timid one duck; a rash driver whose car is shot leaves the lane and ends against what stands by the road; a fleeing driver passes with another coming; a car on rails with a vehicle right ahead of it is let off them; no driver is run down by their own car; a badly damaged car pulls onto the verge out of the traffic's way; a scratched car goes back on rails; a car left in the lane is taken away soon after the player turns their back, stays a while under their eyes and for good with them in it; a jam behind a blocked road clears once the road is open; no more than 24 abandoned cars stand.
- `crates/sim`: a car on rails goes where it is put, and is a car again when let off them.

The tests run on the traffic's fixed dice (`Rng::from_seed` in `Traffic::default`), as every server test runs on fixed ones. Under other dice some of them fail, mostly because they pick a car that then is an EPA or turns off; change the seed and expect to adjust a few.
