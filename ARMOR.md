# Armor, ricochets and the Strv S

What a shot does to a vehicle depends on where it lands and at what angle. The server judges it in
`crates/server/src/mechanics/armor.rs`; the client only draws the result.

## The judgement

A shell, bullet or hitscan round that reaches a vehicle is re-cast against that vehicle's own solids
(`Physics::armor_strike`), which gives the surface normal and the point struck in the vehicle's frame
(+X right, +Y up, -Z forward). From there:

1. The normal's dominant axis is the **zone**: front, side, back or top.
2. A front plate that has a **slope** (`front_slope`) has its normal turned up by that angle. The
   Strv S's glacis leans back 55 degrees, so a level shot straight into it already meets it at 55.
3. The **incidence** is the angle between the shot and that normal. Past the zone's ricochet angle the
   shot **glances off**: a shell is reflected about the normal and flies on at 60 % speed, a bullet
   just stops, and everyone nearby gets a `WEAPON_RICOCHET` shot event (sparks and a ping).
4. Otherwise it does `cos(incidence)` of its damage (never under 25 %), times the zone's multiplier,
   times a **weak spot**'s multiplier if the point is in one (an engine deck, the casemate's roof).

No dice are rolled: the same shot at the same place does the same thing. A shell that lands on a
vehicle does `DIRECT_SHARE` (80 %) of its blast damage scaled by the armor, instead of the damage its
distance from the vehicle's centre would give; neighbours still take the splash.

| kind | front slope | ricochet beyond (front / side / back / top) | multipliers | weak spots |
|---|---|---|---|---|
| Strv S | 55 deg | 58 / 72 / 75 / 78 deg | 1.0 / 1.0 / 1.4 / 1.3 | engine deck x1.6, casemate roof x1.3 |
| tank | 20 deg | 66 / 74 / 78 / 80 deg | 1.0 / 1.0 / 1.35 / 1.3 | engine deck x1.5 |
| cars, helicopter, walker, hen | none | 82 deg all round | 1 | none |
| plane | none | never | 1 | none |

Off the nose by about 22 degrees a level shell bounces off the Strv S front.

## Losing a turret

A hit on a tank's turret solid also subtracts from `Vehicle::turret_hp` (`TANK_TURRET_HP`, 90). At
zero the turret is gone until the tank respawns: its collider is disabled, the gunner dies with it,
the gun cannot fire, byte 4 of its `damage` is `TURRET_GONE` and a `WEAPON_TURRET` shot event tells
clients to throw the turret off as debris. The driver keeps driving.

## The Strv S

`KIND_STRV` (8): one seat. The driver steers with `strafe` (A/D pivots the hull on the spot) and
fires with the fire bit; `pitch` elevates the gun within `STRV_BARREL_MIN..MAX`, and the gun's yaw is
the hull's, so there is no way to aim sideways except by turning. The gun sits in a casemate that is a
weightless solid box on the roof, so the visible housing can be hit. Quicker than the tank
(`STRV_MAX_SPEED` 16 against 12). Client: `render/strvs.ts` (a `TankModel` with no turret node),
built by `tools/vehicles/build_strvs.py`.

Not done: hits on the tracks do not slow a tank yet (the plan kept them as an option).
