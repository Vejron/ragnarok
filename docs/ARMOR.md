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

- A turret's plates are judged in the turret's frame, turned as the gun is. A turret facing the shot
  shows its 50 mm front; turned away, it shows its 30 mm side.
- A plate's `slope` leans its normal back, so a level shot into the Strv S's 55-degree glacis meets
  it at 55 degrees.
- Past a plate's `ricochet` angle a round glances off. A shell is reflected and flies on at 60 %
  speed, and everyone nearby sees sparks (`WEAPON_RICOCHET`). A round that **overmatches** the plate
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
| tank | glacis 50 at 20 deg, hull side 30, rear 20, roof 12, belly 10, engine grille 10; turret front 50, mantlet 60, turret side 30, turret roof 12 | driver's visor (glacis, front left), gunner's sight (beside the mantlet) |
| Strv S | glacis 40 at 55 deg, side 25, rear 20, roof 12, belly 10, engine deck 8; casemate front 40 at 30 deg, casemate side 20, casemate roof 10 | periscope (casemate face, left) |
| Pak 38 | shield 8, sloped 11 deg; carriage 3 | sight gap in the shield |
| jeep, cars, drones | 1.5 sheet steel | none |
| helicopter, walker, hen, mortar | 3 light alloy | none |
| plane | canvas: everything goes through | none |

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
| Carl Gustaf | 400 | rocket: not judged by armour yet, hurts as before |

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
crew member at front left, under the periscope. The Pak 38's crew are in the open; only its breech
is a part.

The other vehicles have parts too, so they behave as a tank does when shot: a hole where every round
goes in, and what it hits inside.

| kind | parts | what a dead engine means |
|---|---|---|
| jeep, sports car, Roadster | engine under the bonnet (120 / 110), fuel tank at the back (100 / 90) | cannot drive |
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
  high explosive's.
- `S_ARMOR` (`Armor`), to the shooter (`HIT_DEALT`) and to the crew (`HIT_TAKEN`): the result
  (`ARMOR_RICOCHET`, `_BLOCKED`, `_PIERCED`, `_SLIT`), the parts hurt and finished off, and the seats
  whose occupant was killed. The client turns these into callouts under the crosshair
  (`client/src/callouts.ts`, `sim/armor.ts`), such as "No penetration ×4", "Penetration", "Engine
  destroyed" or "Driver killed". A bullet through an unarmoured vehicle's skin is called out only if it hit
  something inside; it still leaves a hole.
- Holes and scuffs stay on the vehicle (`client/src/render/hullmarks.ts`). The server's point is on
  its boxes and drums, which only approximate the model. So the client follows the round's path from
  0.8 m before that point to 0.8 m past it into the model's triangles (`render/surface.ts`), and the
  mark goes where the path first meets one: flat on that triangle and parented to its mesh, so it
  slopes with the glacis and turns with the turret or the gun. Failing that it uses the model's
  nearest surface, and failing that the hull box. A hole's rim glows and cools over 5 s. There are
  at most 40 marks a vehicle, and they are cleared when it comes back as new. The sparks, fragments
  and sounds come from the same spot.
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

- Rockets (the Carl Gustaf, the helicopter's) and drones are not judged by armour; they hurt as before.
- Turret crew die only when the turret comes off or a round reaches them; there is no bail-out or
  "knocked out" state for a vehicle whose crew are all dead.
- The AT rifle borrows the sniper's model in hand, and is credited as a sniper shot in the feed.
