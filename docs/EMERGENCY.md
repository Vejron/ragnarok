# Emergency: an ambulance for the casualties, and the police for whoever caused them

A world with traffic can have emergency services. Kill a civilian and the body no longer just fades: it lies there until an ambulance has come and its medics have carried it off. If it was a player who did it, they are *wanted*, and the police come for them: to arrest them at first, and to shoot them once it has gone far enough. A few police cars are out on patrol all the while.

It is built not to run away with itself. There are twenty police cars and two ambulances and never more, each with a crew of two; casualties near each other are one call; the police hurt nobody but the one they are after; and every chase ends by the same rule, that a car whose suspect is wanted no more goes home.

## In play

- **Casualties.** A civilian who is killed lies where they fell. One who is badly hurt (40% of their health left or less, or a leg gone) goes down and stays down, alive. Both wait for an ambulance for up to 90 seconds, and for as long as it takes once one is on its way to them (four minutes at the most). Nobody comes for a hurt civilian in that time, and they get up and run after all. As ever, they are gone once no player is within 150 m.
- **The ambulance** is called 4 seconds after the first casualty, for everyone who lies within 60 m of them in the next two minutes: one ambulance, however many there are. It comes fast with its siren on, parks on the verge by the place, and two medics get out. Each walks to a casualty, kneels by them for 2 seconds, and carries them back; then goes for the next, until nobody is left or it has taken four. They get in and it drives off as traffic, and is gone once nobody sees it.
- **Medics keep out of a fight.** While a shot or a blast has gone off within 40 m in the last 5 seconds, they stay in the ambulance, and those already out crouch where they are.
- **At most 12 casualties** lie waiting at once. Past that, the dead who have lain longest are gone.
- **Wanted.** A crime gives a player heat, shown as one to five stars at the top left of their screen, faint while no police see them (`Vitals::wanted`, protocol 95):

  | Stars | Heat | Police cars | They |
  |---|---|---|---|
  | 1 | 10 | 1 | arrest |
  | 2 | 30 | 2 | arrest |
  | 3 | 60 | 4 | shoot to kill, with rifles |
  | 4 | 100 | 6 | shoot to kill, with rifles |
  | 5 | 150 | 8 | shoot to kill, with rifles |

- **Crimes:**

  | What | Heat | Seen by |
  |---|---|---|
  | Hurting a civilian or a driver of the traffic (once per victim in 10 s) | 10 | anyone |
  | Killing one, or wrecking a car of the traffic with its driver in it | 25 | anyone |
  | Hurting a medic or an officer | 30 | anyone |
  | Killing one | 50 | anyone |
  | Damaging a police car or an ambulance (once in 3 s) | 15 | anyone |
  | Wrecking one | 40 | anyone |
  | Taking one | 30 | anyone |
  | Taking a car of the traffic | 15 | police within 60 m with a clear view |
  | Firing a gun (once in 5 s; alone it never passes two stars) | 10 | police in earshot |
  | A round that passes within 4 m of the police (once in 3 s) | 20 | them |
  | Moving faster than 3 m/s in their sight for 20 s (alone it never passes three stars) | 10 | them |

  Only a player can be wanted. A bot that kills a civilian brings the ambulance and no police.
- **The police come by road** to where the player was last seen, two cars a second until there are as many as the stars call for. The nearest patrol car goes if it can be there in 90 seconds and no fresh one could be there sooner. A fresh one starts near the player, out of their sight: 120 to 220 m off, and further by how far they go in 6 seconds (up to 150 m more), ahead of them within 0.7 rad of the way they go when they go faster than 5 m/s, on a road no more than 45 seconds' drive away (`fleet::start_near`). Out of sight means every person is 350 m off, or at least 100 m off with something between, or with it more than 1.2 rad off the way they look (`traffic::hidden`). Where there is no such place it starts 250 m or more away, out of everyone's sight, as an ambulance does. While they drive they are told every 2 seconds where the player is now, if any of them can see.
- **Those left behind are called off.** A police car more than 250 m from where it looks for its suspect, that has got no 10 m nearer there in 8 seconds and has nobody out of it, is gone at once where nobody sees it go, and else drives off (`FleetOrder::Recall`); another comes from ahead in its place. An officer more than 150 m from where the police last knew the suspect to be, where nobody sees them, is gone.
- **They leave the road for you.** A police car within 150 m of a player the police can see goes after them across fields: within 80 m by the way the soldiers' grid finds round buildings, heading for the furthest corner of it within 30 m that it has room to drive straight to, and round whatever it feels ahead with a fan of rays as wide as the car (`mechanics/steering.rs`), slowing for what is ahead and for sharp turns. Water too deep to wade and steep banks count as in the way. One who drives is chased flat out and rammed, on the boost while it is more than 20 m behind and heading within 0.3 rad of them. One on foot, or in a vehicle that has all but stopped, it brakes hard (6 m/s²) to pull up 10 m short of, and its chase is over once it is within 8 m of that and slower than 6 m/s. A car that gets stuck on the way (three goes at backing out) stops where it is. Out of sight or further off, it goes back to following by road.
- **Pulling over.** A police car on its way that comes within 50 m of where it looks for the player pulls onto the verge where it is (`FleetOrder::Pull`): as far on as it takes to brake from its speed at 7 m/s², 12 m at the least. So cars that come together each stop where they are, rather than queue for the one spot on the road nearest the player. One counts as parked once it is within 6 m of its spot and slower than 3 m/s.
- **Officers get out** where the car has pulled up or got stuck, or parked on the road within 60 m of the player, as it rolls to a stop: the police look at their cars five times a second (`DISPATCH_EVERY`), and send more and call off those left behind once a second. The two of them run for the player, or for where they were last seen. They climb out by the front doors and leave them standing open (DOORS.md): the two who get out are the two who drove there. With eight cars coming in together, each has its officers out within about 5 seconds of coming within 60 m, the last second of it climbing out.
- **A police car or an ambulance with its crew in it cannot be taken**: nobody drags an officer out. One they have left parked can be, and that is a crime.
- **An arrest** (one and two stars) takes an officer within 3 m with a clear view, and the player keeping still (under 1.5 m/s on foot, under 2 m/s in a vehicle, or knocked down) for 2 seconds. Moving off breaks it. Arrested, the player is out of their vehicle, has what a fresh life carries, is fined a fifth of their cash, and starts again where a life starts. It is not a death.
- **From three stars** the officers fight as soldiers do (BOTS.md): each has a bot's mind (`Bot`, skill 0.5, courage 0.7 or more) that takes over (`Officer::armed`). They close in bounds, kneel to fire, move between spots, take cover to reload or when hit, and with two on one suspect one goes round the side. They see, hear and turn to their suspect alone, never give in, and take no room among a scenario's bots (`MAX_BOTS`). The one who gets out by the second seat carries a Carl Gustaf as well (`officer_loadout`), and uses it on a suspect in a tank, never on anyone else nor on a car; the other gets out of a tank's way, as anyone without an anti-tank weapon does, and fires on it now and then from there. The officers' own mind takes them back for the arrest, the search and the walk to the car.
- **Getting away.** The police see a player within 80 m with nothing between them, officers and cars alike. Unseen for 20, 30, 60, 90 or 120 seconds (by the stars), a player is wanted no more. From three stars, once the player has been out of sight 3 seconds, each car searches its own spot ahead of where they were going: as far on as they could have gone since at the speed they were last seen at (up to 150 m), and up to 60 m to one side or the other. The time only counts once they are 50 m from where they were last seen, or a police car has got there and not found them: standing at the scene waiting for the police is not getting away. Officers who lose sight of the player go to where they were last seen, give it 20 seconds, and walk back.
- **It ends** when the player dies, is arrested, gets away or leaves the game. The officers walk back to their car, get in, and it drives off; a police car joins the patrols if they are a car short, and is otherwise gone once nobody sees it.
- **Traffic makes way.** Cars up to 80 m ahead of a siren pull onto the verge and stop, and drive on 2 seconds after it has passed. A car with its siren on drives at one and a half times the road's speed, is given every junction, and passes what has not got out of its way.
- **The cars** are a white Volvo 240 and a yellow Transporter van, each with a light bar that flashes and a two-tone siren while it is on its way, and a mark on the minimap ([VEHICLES-XXV.md](VEHICLES-XXV.md)).
- **Patrols** are police cars driving as traffic does, with no siren. A scenario says how many (twenty at the most).

## What keeps it orderly

1. **A fixed fleet.** Twenty police cars, two ambulances, two people in each. A unit nobody is near is a row of the traffic's table on the road network and nothing more, so the number costs nothing until they are near a player. A unit that is wrecked, taken or stuck is lost, and its place is free for the next call; an ambulance that was on its way is replaced after 20 seconds.
2. **One incident, one ambulance.** A third place with casualties waits while both are out, and its dead are gone after their 90 seconds.
3. **An officer's rounds are for their suspect alone.** They take no health off anyone else, nor off a vehicle the suspect is not in (`emergency::misses`), so police fire makes no casualties and no bystander is ever dragged into it.
4. **A police car or an ambulance on its way knocks down whoever it hits and takes no health for it**, unless that is someone the police are after (`emergency::spares`).
5. **Each suspect has their own cars**, the most wanted served first, and a car keeps its suspect.
6. **The services are nobody's enemy and nobody's fright.** Bots do not shoot at them, civilians do not run from them, drivers are not frightened by their guns, and they do not hurt each other.
7. **One rule for going home**: a unit whose suspect has no heat leaves. Nothing else has to remember to call it off.

## How it is made

`Traffic` drives; `Emergency` commands. They share `World::fleet`: orders one way (`FleetOrder::Send`, `Redirect`, `Release`), and where each unit is the other (`Unit`).

- **The fleet** (`traffic/fleet.rs`) is twenty-two rows ahead of the traffic's in the same table, so a police car is on rails where nobody is near, a vehicle where someone is, and physical beside them, as any car of the traffic (TRAFFIC.md). A row has a `Duty`: `Dormant` (on no road), `Patrol`, `Responding`, `Chasing` (off its lane, driven straight at a player by `fleet::chase`), `OnScene` or `Leaving`. A unit off the road keeps looking up the road nearest it, and drives back onto that one when it is sent on or let go.
- **Finding the way** (`traffic/route.rs`). Traffic wanders; a unit is sent somewhere. `Graph::locate` finds the road nearest a place from a 64 m grid of the edges, preferring one on the level of the place to a bridge over it. `Graph::route` is Dijkstra over the *ways* (an edge driven one way), by time at each road's speed, so it never turns straight back and a place just behind is reached round the block. On `home` (16,000 ways) a route takes 0.1 ms, the slowest of 200 0.3 ms. The plan is a queue of ways the row takes at each junction in place of a random one; off its plan, or with a road ahead shut, it plans again, and with no way left it is lost.
- **Three moods** join the drivers' (`traffic/drivers.rs`): `Responding`, `Parked`, and `Yielding` for the traffic a siren clears. A parked unit is off its lane, so nothing queues behind it.
- **Casualties** are state on the civilian (`Civilian::hurt`, `claimed`), not another entity.
- **Incidents and suspects** (`mechanics/emergency.rs`) are looked at once a second. A unit is sent *for* something, its tag: an incident's number or a suspect's id, which is how the two sides know whose a unit is.
- **The crews** are at the wheel only as the traffic's drivers are, a light entity, two of them: the driver in seat 0 and their mate in seat 1. Parked, each climbs out by their door (`Emergency::put_out`, DOORS.md), and once out is turned, under the same id, into a player with no connection (`Medic`, `Officer`) by the room, as civilians are (`EmergencyCall`), with minds of a few states each (`emergency/medics.rs`, `officers.rs`) that walk by the bots' pathfinding. Back at a free door they climb in and are its crew again under the same id (`EmergencyCall::Aboard`), and the car drives off with them. A car that nobody saw into as it came has nobody in it, and its crew appear at its doors.
- **Heat** is a `Wanted` component on the player (`emergency/wanted.rs`), judged from the `Damaged` events that already name the attacker, from the noises shots make, and from who sits at a wheel they should not.

## Turning it on

| Where | What |
|---|---|
| `scenario.json` | `"emergency": { "patrols": 2, "police": true, "ambulance": true }`, or **Police and ambulances** under Streets in the editor's Scenario panel (it needs traffic) |
| `RAGNAROK_EMERGENCY=n` | police and ambulances with `n` patrols, whatever the scenario says |

It needs roads to drive (200 m of driven lane, as traffic does), and works with the traffic itself turned off. In a world with none, the dead lie their 10 seconds as before.

## Not there yet

- Officers do not shoot from the car: a player who never slows down is only chased and rammed.
- The grid a chasing car finds its way by is the soldiers', which knows nothing of how wide a car is: it scrapes a corner now and then, and backs out of what it sticks on; after three goes its officers walk.
- The second of a crew is not seen in the car, only the one at the wheel.
- A casualty is gone from the ground when a medic has lifted them; nothing is seen carried.
- Road blocks, helicopters, and police on foot patrol.
