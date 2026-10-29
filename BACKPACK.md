# The backpack

A game mode for any world: one **Intinor Compact Backpack**, a piece of technological marvel full of valuable electronics and priceless code, for everyone to fight over. There is no round and no end: whoever holds it scores, and everyone else wants it.

## Turning it on

Put a top-level `backpack` in the world's `scenario.json` (see [SCENARIOS.md](SCENARIOS.md)), where it should start:

```json
{ "backpack": { "lat": 63.8400, "lon": 20.3150 } }
```

It works on its own or alongside squads and triggers. A position outside the world stops the server at start-up, as any other mistake in the file does. Without it the world has no backpack and nothing below happens.

## Rules

- **Taking it.** A soldier on foot who walks onto it (within 1.5 m) takes it. No key is needed. Someone in a vehicle cannot pick it up, but a carrier who gets into one keeps it.
- **Carrying it.** The carrier heals 5 health a second, and walks and runs twice as fast on foot. It does not grow lost limbs back. They score a point (a second) for each second they hold it: the scoreboard's **Backpack** column, as m:ss.
- **Losing it.** It falls where the carrier dies, or where they were when they left the game: on the floor under them, indoors too. If there is nowhere to put it (off the world), it goes back to its start. Lying untouched for 60 s away from its start, it goes back too.
- **Being found.** Its six 5G modems give it away. Everyone's minimap, the carrier's included, shows an orange circle with the backpack somewhere inside it, not at its middle. It is 150 m across while it lies about or has just changed hands, and narrows to 20 m over 5 minutes in one carrier's hands. When the backpack nears the circle's edge, a new circle is drawn around it. A circle off the small map shows as an orange pip on its edge, pointing the way.
- **Bots** do not go for it, but they shoot its carrier like anyone else.

## Tuning

Every number is a `BACKPACK_*` constant in `crates/shared/src/consts.rs` (run `npm run gen:consts` in `client` after changing one): `BACKPACK_SPEED` (2), `BACKPACK_REGEN` (5 hp/s), `BACKPACK_WIDE` and `BACKPACK_NARROW` (150 and 20 m across), `BACKPACK_NARROW_SECONDS` (300), `BACKPACK_REACH` (1.5 m) and `BACKPACK_RETURN_SECONDS` (60). How far off the middle a new circle may be (0.6 of its radius) and how near the edge it is redrawn (0.9) are `OFFSET` and `EDGE` in `crates/server/src/mechanics/backpack.rs`.

## How it works

- **Server.** The `Backpack` mechanic (`crates/server/src/mechanics/backpack.rs`) keeps `World::backpack`, which is `None` without the mode. Lying, the backpack is an entity with `Lying`, `Transform` and `kind` 15. Carried, it is a `Carry` component on the carrier, which `Movement` reads for the speed. The mechanic heals and scores the carrier, drops the backpack on a `Died` (in `react`) or on leaving, and draws the circle. The circle's middle is the backpack's place plus a random offset, so the server never sends where the backpack is. Players near it see it in the snapshot like anything else in range.
- **Wire.** The `backpack` message (tag 18) carries the circle, the carrier and their held seconds. It goes to everyone once a second and whenever the circle moves or the backpack changes hands, and to a joiner. The `carry` component (22) marks the carrier, `Score` carries `held`. Protocol 52.
- **Client.** `BackpackTracker` (`client/src/backpack.ts`) keeps the circle and says when the backpack changes hands. The carrier's own prediction multiplies its speed by `BACKPACK_SPEED` while their `carry` is set, so the speed is predicted, not corrected. `Minimap.vue` draws the circle (`circleOnSmall` in `sim/minimap.ts`). `entities/presenters/backpacks.ts` and `render/backpack.ts` draw the backpack lying about, and `Character.pack` draws it worn.
- **Tests.** `mechanics/backpack/tests.rs` covers taking it, healing, twice the speed, dropping it on death and on leaving, going home, the circle narrowing and always holding the backpack, and the score. On the client, `backpack.spec.ts`, `minimap.spec.ts` and `scoreboard.spec.ts` cover the tracker, the circle's placement and the held column.
