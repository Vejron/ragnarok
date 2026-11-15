# Hull damage: what a round leaves on a vehicle

A round that strikes a vehicle should look as if it did something, even on a Titan that it barely
hurts. It leaves a mark that stays, and the blow rocks the vehicle. Both work from what the server
already sends: a `HullMark` gives the point in the vehicle's frame, the round's way and its style
([ARMOR.md](ARMOR.md)).

## Scars

Each mark becomes a **scar** on the mesh it struck. The scar is not a decal mesh: the mesh's own
shader works it out ([RENDERING.md](RENDERING.md), "Scars"). A scar is a point in the mesh's frame
with a reach, the round's way, a style and its time of birth.

| Mark | Scar (`SCAR_*`) | Looks like |
|---|---|---|
| A hole (`MARK_HOLE`, `MARK_PIERCED`) | dent, hole, soot, hot | A bowl pressed in, black at its middle, ringed with tempered steel (blue by the hole, purple and bronze to straw at the edge). It glows white-orange and cools through red over 7 s, and soot rises up the plate from it |
| A shell stopped (`MARK_SCUFF` + `MARK_HEAVY`) | dent, soot, hot | A deeper dent with a lip, paint flaked off its rim down to the metal, and a scorch |
| A shell glancing off (`MARK_GLANCE` + `MARK_HEAVY`) | gouge, soot, hot | A groove torn along the way the round went, bare bright metal with fine lines along it that glint in the sun |
| A bullet (no `MARK_HEAVY`) | gouge, dent | A chip in the paint. Chips are kept in a ring of their own, so a hail of them never wipes out a shell's dent |
| A blast beside it (client side) | soot | Soot on the faces nearest an HE burst that went off within its radius of the vehicle |

- **Size.** How far a scar reaches is set by the mark (`LOOKS` in `hullmarks.ts`), by the round
  (a shell through is bigger than a bullet through a slit) and by the vehicle's size (`vehicleSize`
  in `events.ts`, 1 for a tank and about 2.6 for a Titan). A shell's hole on a Titan is about 2 m
  across, its soot twice that.
- **Grime.** As a vehicle loses health, its scarred parts are dulled and sooted all over, and a
  burnt-out wreck most of all.
- **They stay.** A mesh keeps 18 scars and 6 chips, the oldest giving way. They are cleared when the
  wreck comes back as new.
- **Real dents.** Where the GPU allows it (`Renderer.vertexScars`), the vertices are pressed in as
  well as shaded. The Titan's big plates are cut into a 1.2 m grid for this (`DENT_GRID` in
  `build_titan.py`). Other vehicles' coarser meshes show their dents mostly in the shading.

## The blow

- **On the server** (`Physics::strike_car`). A shell, rocket or cannonball that strikes a vehicle
  pushes it where it struck, at its `Burst::hit_knock`: 0.6 m/s for an AP shell, 0.4 m/s for HE and
  0.15 m/s for an autocannon round. A glance gives 0.3 of that. Because the push is at the point, a
  tank hit high on its side rocks away from the hit. A Titan takes its `blast_share` of the push and
  its `hit_share` (4.5) of the turn. An AP shell on its chest rocks it about 2°, and it stands still
  again in under 1.5 s. Bullets push nothing.
- **On the client, at once** (`render/flinch.ts`). The server's push reaches the picture a snapshot
  later, so the model also flinches as the mark lands: a damped spring about the ground under it,
  kicked about the line from there to the hit. It runs at about 3 Hz for a tank and 1.7 Hz for a
  Titan, plus a 24 Hz shiver of the metal for 0.07 s. A Titan's legs bend to keep its feet planted.
  A hit on its arm swings that arm back (`TitanModel.recoil`).
- **The crew** feel a shell's hit through the camera's shake, whatever damage it did. A Titan's
  armour takes so little that the old damage-scaled kick was nothing. The Titan's own pilot is not
  flinched, since the visor would swing through the view. A headset's view stays level anyway: it
  takes only the Titan's heading.
- **Sparks** scale with the round and the vehicle: a shell stopped by a Titan's plate throws fire,
  a gout of smoke and sparks to match.

## Where it lives

- `engine/src/scene/scars.ts`, `gpu/scars.ts`, `materials/scars.wgsl.ts`, and the scar block in
  `materials/standard.wgsl.ts`
- `client/src/render/hullmarks.ts` (marks to scars), `render/flinch.ts`, and `events.ts` (`showMark`,
  `scorchNear`)
- `crates/sim/src/vehicle.rs` (`strike_car`) and `crates/server/src/mechanics/projectiles.rs` (the knocks)
- `crates/server/src/mechanics/armor.rs` (`MARK_HEAVY`)

## Cost

All timings are at 2268×1258, alternated within one run:

- A Titan scarred all over and filling the frame: 0.3 ms with its scars against without.
- The denser Titan model: 0.11 ms against 0.05 ms for the old one.
- Placing a mark (following the round into the model's triangles): about 1.3 ms of CPU on the
  denser Titan. Bullets' marks are limited to one per vehicle a snapshot.

## Tests

- `engine/test/scars.test.ts`: the slots, the heavy and light rings, and running out of slots.
- `client/src/__tests__/flinch.spec.ts`: a struck vehicle rocks and comes to rest, and a Titan rocks
  less than a tank.
- `client/src/__tests__/armor.spec.ts`: the vents smoke comes out of still follow the holes.
- The server's knocks and the Titan's hit shapes: [VEHICLES-XIV.md](VEHICLES-XIV.md), "Tests".

## Not yet

- Dents are not in the shadow pass, so a dent's own shadow is the plate's.
- Other vehicles' meshes are not cut finer, so their dents are mostly shading.
