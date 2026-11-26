# Armour, penetration and parts

What a round does to a vehicle depends on which plate it meets, at what angle, and how much steel it
can go through. Once through, it goes on into the parts and the people inside. The server decides all
of it (`crates/server/src/mechanics/armor.rs`); the data is in `crates/shared/src/armor.rs` and each
kind's definition (`crates/shared/src/vehicles/<kind>.rs`). The client draws the result and shows the
shooter and the crew what happened. No dice are rolled: the same shot at the same place always does
the same thing.

## Plates

A kind's `Armor::plates` is an ordered list. A round that reaches a vehicle is re-cast against that
vehicle's own solids (`Physics::armor_strike`), which gives the solid it hit (the hull, or one of
`spec.solids`), the surface normal and the point, all in the vehicle's frame (+X right, +Y up,
-Z forward). The first plate that covers that solid, face (front, side, back, top, belly) and region
is the one met. Small, special plates (a visor slit, the mantlet, an engine grille) are listed before
the plain faces.

A tracked hull is not a box but its real side outline (`Spec::profile`, see
[VEHICLES-XVI.md](VEHICLES-XVI.md)), and a round meets each face of it as it lies. The face is told
from the real normal, so a glacis that lies flatter than 45 degrees is struck as *top*: its plate is a
`Zone::Top` plate with a region over the glacis, listed before the plain roof. The plate under a
nose is told apart from the belly the same way.

- A turret's plates are judged in the turret's frame, turned as the gun is. A turret facing the shot
  shows its 50 mm front; turned away, it shows its 30 mm side.
- A hull with an outline slopes as it is drawn: a level shot from ahead meets the tank's glacis at
  57 degrees and the Strv S's at 77. A plate's `slope` leans the normal of a plate on an upright solid
  back instead (the Strf 90's turret face), which has no slope of its own.
- **The Strv S turns most shots from ahead.** Its glacis is half its length and lies 13 degrees above
  level, and the plate under its nose leans far back too. A level shell skids off either, from dead
  ahead and from the front quarters. A shell coming down at 12 degrees bites but has 107 mm to get
  through and is stopped; from 25 degrees up it goes through. Its sides, tail and decks are thin, and
  the gunner's sight in the glacis is a slit.
- Past a plate's `ricochet` angle a round glances off. A shell is reflected and flies on at 60 %
  speed, and the plate shows a gouge and sparks (`MARK_GLANCE`). A round that **overmatches** the plate
  (penetration of at least three times its thickness) bites in at any angle.
- Otherwise it has to get through `mm / cos(incidence)`. A round that cannot is **blocked**: a scuff
  and a ping (`MARK_SCUFF`), and no damage. An HE shell's blast still does 15 %.
- A round that gets through is **pierced**. Solid shot does between half and all of its damage,
  depending on how much penetration it had to spare; HE does all of it. Everyone sees a hole and
  fragments (`MARK_PIERCED` for a shell, `MARK_HOLE` for a bullet).
- A **slit** (`Plate::slit`) is a gap, not a plate. A round goes straight through it, does no harm to
  the vehicle itself, and throws no fragments: it hits whoever is behind it.
- Splash from a blast that misses does a fifth of its damage to an *armoured* vehicle (every plate
  at least 8 mm), and can still break a track it bursts beside.

| kind | plates (mm) | slits |
|---|---|---|
| tank | glacis 32 leaning back 57 deg (59 along a level shot), lower front 45, hull side 30, rear 20, roof 12, belly 10, engine grille 10; turret front 50, mantlet 60, turret side 30, turret roof 12 | driver's visor (glacis, front left), gunner's sight (beside the mantlet) |
| Strv S | glacis 40 leaning back 77 deg (180 along a level shot), lower front 40 leaning back 57 deg, side 25, rear 20, roof 12, belly 10, engine deck 8 | gunner's sight (glacis, left of the gun) |
| Strf 90 | engine cover 8 leaning back 76 deg (33 along a level shot), lower front 30, side 18, rear ramp 10, roof 10, belly 8; turret front 30 at 15 deg, turret side 20, turret roof 10 | driver's periscope (engine cover, front left), gunner's sight (beside the gun) |
| Pbv 302 | glacis 23 lying 38 deg above level (38 along a level shot), lower front 23, side 14, rear doors 10, roof 8, belly 8; turret 12, turret roof 8 | driver's periscope (glacis, middle), gunner's sight (the turret's face) |
| Pak 38 | shield 8, sloped 11 deg; carriage 3 | sight gap in the shield |
| jeep, cars, drones | 1.5 sheet steel; a car's cabin is glass (`GLAZED`) | none |
| technical | 1.5 sheet steel; its gun's receiver 6 | none |
| helicopter, walker, hen, mortar | 3 light alloy | none |
| plane | canvas: everything goes through | none |

## What a round can strike

A car's shape in the physics is its hull box, which reaches only up to the beltline: the cabin
above it was never there, and a round through a window flew on as if the car were not. Rounds now
also meet solids that are only there for them (`Role::Shot`):

- **Every glazed car has its cabin**: the traffic cars, the police car, the EPA, the ambulance and the
  Transporter. It is a `Shape::Glasshouse` (`Solid::cabin`) from the hull's top to the roof, whose
  windscreen and back glass lean as the model's do and whose sides lean in towards the roof, measured
  off each model's glass. It is glass all round (`Plate::glass`, the kinds' armour `GLAZED`), roof
  included: a round goes straight through it as through a slit, breaks it, and hits whoever sits
  behind it. The car itself takes no harm and throws no fragments, and nobody is told unless the round
  went on to hurt a part inside (a Ball round stops in the first it meets). A shell bursts on it as on
  any plate.
- **The glass breaks where a round went in**, even when the round went on to hit someone in the seat
  behind it: a round that met a car first and a body beyond it leaves its hole in the car where it went
  in (`combat::land`), and does the car no other harm. That holds for a glazed car's doors too. On the
  model a bullet's hole in a glass mesh is broken glass (`SCAR_CRACK`, scars.wgsl): a dark hole crushed
  white round its edge, a star of cracks running out from it and a broken ring of them round it, drawn
  opaque on the see-through glass. It is the same size whatever the round, nothing smokes out of it, and
  it sounds as a window pane breaking does, with no sparks.
- **The technical's gun has its receiver**, a drum round the gun's pivot that holds it at any bearing,
  behind 6 mm of steel. A revolver's round is stopped; a rifle's goes through up close, and two of them
  break the breech, so the gun cannot fire.

These solids are not in the physics. A car drives, rolls and passes under a bridge as before, and a
soldier still cannot stand on its roof. Only rays that stand for a round meet them: `Physics::cast_round`
(a hitscan round, and bullets and shells in flight) and `Physics::armor_strike`. A round that starts
inside one, from a soldier who has climbed into a cabin, goes out through it. The bots' line of sight,
the floor that dropped things land on and the client's aim lines use the plain ray, as before. A round
is tested against these solids only for a vehicle it passes near, so it costs next to nothing.

The jeep, the Roadster and the technical's cab are open at the sides and have no cabin: a solid there
would shield their crew from rounds that should reach them. The bus's body is already a solid of its
own, in the physics.

## Rounds

Each round has a `Pen`: millimetres at the muzzle, falling to a lower figure at a given range, a
kind (`Ball`, `Ap`, `He`) and a calibre. Guns carry it on their `ItemDef`, machine guns on
`Fire::Hitscan`, and shells by projectile kind (`projectiles::pen_of`).

| round | penetration (mm) | notes |
|---|---|---|
| revolver | 4 to 2 at 150 m | ball |
| AK5 | 8 to 5 at 300 m | ball: nothing but slits on a tank |
| sniper | 12 to 9 at 400 m | AP: the Pak's shield, a tank's slits |
| AT rifle | 40 to 30 at 300 m | AP: a tank's hull and turret sides and rear (not its front), the Strv's sides and casemate, the Pak's shield |
| plane MG | 10 to 7 at 300 m | ball |
| tank, Strv, Pak: AP (`PROJECTILE_SHELL`) | 90 | solid shot with a small burst (3 m, 70); kills a person it strikes |
| tank, Strv, Pak: HE (`PROJECTILE_HE`) | 15 | the old big burst (7 m, 125); for infantry, soft vehicles and roofs |
| helicopter autocannon | 20, HE | goes through a tank's roof |
| mortar | 15, HE | |
| bomb | 60, HE | |
| Carl Gustaf | 100, shaped charge | see below |

**The Carl Gustaf's rocket** carries a shaped charge (`projectiles::Shaped`, the item's `pen`): it is judged
by the armour it meets, as a shell is, but never glances off: a plate it would have skidded off stops
it. What it does to the hull goes with what it had to spare (`armor::shaped_share`): `residual / 100` of
its direct hit (0.8 × 160), and never less than a plain rocket's blast does to armour (`SPLASH_ARMOURED`,
0.2), stopped or only just through. Inside, its jet goes on with a twentieth of what it had to spare
(`SHAPED_INSIDE`) and does 40 to a person in its way (`SHAPED_CREW`). Hits to wreck one, from 30 m:

| | front | side | rear |
|---|---|---|---|
| tank (60 mm front) | 5 | 3 | 3 |
| Strv (its sloped glacis, 180 mm along a level shot) | 9 | 3 | 3 |
| Strf, Pbv, AMV | 2 | 2 | 2 |
| Titan | about 85 (3 a hit) | about 85 | about 40 (6 a hit) |

The Titan's toughness (`Body::tough`, 0.1) cuts this as it cuts everything. A tank's driver lives
through the first hit from ahead. `armor/tests.rs` fires the rockets (`carl_gustaf_hits`).

A gun with two kinds of round (`Fire::Shell::alt`) swaps them with Q (`BUTTON_AMMO`, one swap per
press). The swap takes a reload, and `TURRET_ALT` tells the client which round is loaded: the HUD
shows "AP" or "HE".

## Parts and crew

`Armor::modules` places a kind's parts as boxes in the hull frame. Those marked `in_turret` are
placed in the turret's frame, turn with it, and go with it when it is shot off. Each part's health
is on `Vehicle::parts`. Its state (sound, hurt at half or less, gone) is packed two bits each into
slots 8 and 9 of the replicated `damage` (after a car's four wheels), and slot 10 says the fuel is
burning. That way the driver's
own prediction (`CarCondition::of`, which runs on both server and client) sees the same mobility as
the server.

- A pierced round goes on along its path inside (up to 8 m) and hits each part and crew member in
  its way. The first takes `30 + 1.5 × spare mm`, and each one after that 70 % of the one before,
  until the round is spent. A ball round stops in the first thing it meets.
- A round of 20 mm or more that went through a plate also throws a fixed fan of 2 to 8 fragments,
  in a 26-degree cone. Each fragment hits the first thing it meets: 15 + spare/5 to a part, 45 to a
  person. Fragments can hurt the ammunition but never set it off; only the round itself does that.
- A crew member hit is hurt through the normal damage path, with the round's weapon, so the shooter
  is credited. A shell kills; a bullet through a slit does its own damage.
- A shell of 20 mm or more that bursts on or within 1.5 m of a track hurts it, whether or not it
  got through the armour.

| part | hurt | gone |
|---|---|---|
| engine | 60 % power and top speed | no power: cannot move or pivot |
| transmission | 70 % top speed | no power |
| track (left, right) | 80 % top speed | that side is not driven and drags; 30 % speed, 35 % turn. Both: immobile |
| breech | | the gun cannot fire (`Need::Breech`) |
| ammunition | | cooks off: the turret is thrown and the vehicle is wrecked |
| fuel | | burns (`Burning`): 6 health a second until it is a wreck |

The tank's parts are, from front to back: the transmission in the nose, the driver at front left,
the ammunition along the right of the fighting compartment, the fuel ahead of the engine, and the
engine at the back under the grille. The breech and the gunner are in the turret. The Strv S has its
engine and gearbox at the back, its fuel under the middle, its magazine behind the crew and its one
crew member on the left, behind the gunner's sight. The Pak 38's crew are in the open; only its breech
is a part.

The other vehicles have parts too, so they behave as a tank does when shot: a hole where every round
goes in, and what it hits inside.

| kind | parts | what a dead engine means |
|---|---|---|
| jeep, sports car, Roadster | engine under the bonnet (120 / 110), fuel tank at the back (100 / 90) | cannot drive |
| technical | engine under the bonnet (130), fuel under the bed (100), the gun's receiver (60) | cannot drive; with the receiver gone, the gun cannot fire |
| helicopter | turbine over the cabin (110), fuel under the floor (90) | the rotor stops and it falls |
| plane | engine in the nose (90), fuel ahead of the cockpit (70) | it glides |
| walker, hen | the pilot in the sealed cab, fuel at the back (100) | (no engine) |

A rifle round through a car's skin does `30 + 1.5 × spare mm` (about 39 from an AK5) to the part
it reaches, so it takes about four rounds into a bonnet to stop a jeep and three into its fuel tank
to set it alight. The crews of the open and glazed vehicles are not parts: they are hit as anyone
is, through the glass. The drones, the loitering munition and the mortar have no parts.

A wreck coming back as new (`Vehicle::renew`) has every part whole and its fire out.

## Telling the players

- `S_MARKS` (`Marks`), sent with each snapshot to the players near: every `HullMark` since the
  last. A mark has its style (`MARK_GLANCE` sparks, `MARK_SCUFF`, `MARK_HOLE`, `MARK_PIERCED`), the
  point in the vehicle's own frame and the round's direction in that frame. The client shows it when
  the picture reaches that tick, on the vehicle as it is drawn then. That keeps it on a moving tank,
  which a world point would miss by its speed times the interpolation delay. A round under 20 mm that
  glances off or is stopped leaves a mark only if that vehicle has no glance or scuff waiting for
  the snapshot yet: a squad's rifles on a tank make one scuff per snapshot, not dozens. Holes and
  shells always leave one. `WEAPON_AP` is an AP shell's burst (a shot event); `WEAPON_SHELL` stays
  high explosive's. A glance or a scuff from a shell or a cannon (20 mm or more) also has
  `MARK_HEAVY`, so the client draws a dent worth keeping rather than a chip in the paint.
- `S_ARMOR` (`Armor`), to the shooter (`HIT_DEALT`) and to the crew (`HIT_TAKEN`): the result
  (`ARMOR_RICOCHET`, `_BLOCKED`, `_PIERCED`, `_SLIT`), the parts hurt and finished off, and the seats
  whose occupant was killed. The client turns these into callouts under the crosshair
  (`client/src/callouts.ts`, `sim/armor.ts`), such as "No penetration ×4", "Penetration", "Engine
  destroyed" or "Driver killed". A bullet through an unarmoured vehicle's skin is called out only if it hit
  something inside; it still leaves a hole.
- Marks stay on the vehicle as scars on the mesh they struck (`client/src/render/hullmarks.ts`,
  [HULL-DAMAGE.md](HULL-DAMAGE.md)): a dented, sooted hole ringed with blued steel that glows and
  cools, a scorched dent where a plate stopped a shell, a gouge to bare metal where one glanced off,
  and a chip in the paint for a bullet (broken glass, on a car's glass). The server's point is on its hull outlines, boxes and drums, which only
  approximate the model. So the client follows the round's path from 0.8 m before that point to
  0.8 m past it into the model's triangles (`render/surface.ts`), and the scar goes on the mesh the
  path first meets, in that mesh's frame, so it slopes with the glacis and turns with the turret, the
  gun or a Titan's arm. Failing that it uses the model's nearest surface. A mesh keeps 18 scars and 6
  chips, the oldest giving way, and they are cleared when the vehicle comes back as new. The sparks,
  fragments and sounds come from the same spot.
- Each slit is drawn as a dark gap on the model (`entities/presenters/slits.ts`, from
  `Armor::slits` in the tables). It goes where a round going straight in through the slit's box crosses
  the model, so a level shot at the gap goes through it. On a shallow plate such as the tank's glacis,
  the gap is stretched to the strip that level shots cross. While the seat behind the slit is taken
  and the vehicle lives, a faint glint flickers in it. The crew themselves are not drawn.
- A hit engine smokes, a dead one smokes black, and burning fuel burns like a wreck and lights the
  ground round it (`entities/presenters/smoke.ts`). It comes out of three holes, newest first, and
  first of all the holes whose round hurt a part inside (`MARK_VITAL` on the mark's style)
  (`HullMarks.vents`, which follow the hull and turret as drawn and drop a hole on a turret thrown
  off), or out of the engine bay if the vehicle has none. Once it burns, each hole has small
  fireballs licking out of it and throws tiny glowing embers on arcs like a blast's. A fresh hole
  throws them hard for its first 6 s, then settles to a trickle (`render/vehiclesmoke.ts`).

## Not done

- The helicopter's rockets and drones are not judged by armour; they hurt as before.
- Turret crew die only when the turret comes off or a round reaches them (a T-55's machine gunner, in
  the open on the roof, can also be shot like anyone else); there is no bail-out or
  "knocked out" state for a vehicle whose crew are all dead.
- The AT rifle borrows the sniper's model in hand, and is credited as a sniper shot in the feed.
