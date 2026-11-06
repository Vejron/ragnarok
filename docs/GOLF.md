# Golf

Any world with a golf course in OpenStreetMap can be played. Kläppudden has Norrmjöle golfbana. The importer lays out the course: fairways, greens, tees, bunkers, and a flag in every cup. The server keeps a ball waiting on every tee. Pick up a club and play.

## Playing

- **Clubs.** There are two: the **Driver** (item 14) and the **Putter** (item 15). They are hotbar items like any other: lay them out in the editor (`area.json` `items`), or start with them in a loadout. They are never scattered. At Kläppudden a pair lies beside hole 1's tee.
- **The golf cart** (`KIND_GOLF_CART`, 22) seats two side by side, the driver on the left. It is electric and slow, topping out at 12 m/s, and too slow to skim water, so it sinks and wades. Place one in the editor (G). At Kläppudden two are parked beside hole 1's tee, facing up the fairway. Its tuning is `crates/shared/src/vehicles/golf_cart.rs`, and its model is `build_golf_cart` in `tools/vehicles/build_car.py`.
- **Swinging.** Stand still with a club and you take your stance over the ball: side-on, with the target (where you aim) on your left and the ball by your right foot. A swing takes three clicks on the meter over the hotbar:
  1. **Press fire and hold it.** The power fills the bar, reaching a full swing in 1.2 s. Held on, it runs into the red **overswing**, up to 110%, and lets go by itself at the top.
  2. **Let go** to set the power. The club is at the top of the backswing.
  3. **A marker runs back across the bar** to the line near its left end (a full bar in 0.9 s), and the club starts down. **Press fire again on the line.** The zones around the line say what you get:
     - **White** (±1 frame) is pure: dead straight.
     - **Green** is a draw (early) or a fade (late): a gentle curve left or right.
     - **Amber** is a hook (early) or a slice (late): a hard curve, fully bent 7 frames off.
     - **Red** (more than 9 frames off, or never pressed) is a mishit, topped. It leaves low, with 40% of the power, and curves hard: smothered left if early, right if never pressed or late.
  - **Curving on purpose.** Click a little before the line to draw the ball round something on the left, or a little after to fade it right.
  - **Overswinging.** It goes further, but any curve is doubled and the ball sprays a little off line by chance.
  - **The putter.** A putt has no flight to curve in, so mistiming it pulls the putt left or pushes it right instead, by up to 0.1 rad.
  - Letting go strikes the nearest ball within 1.6 m of your feet that is not behind you, and sends it the way you aim. After a strike the meter rests for 1 s.
- **The driver** sends the ball at up to 70 m/s, 12° up. With aim held it pitches it high (43°), so it stops soon after it lands. **The putter** rolls it along the ground at up to 6 m/s.
- **The ball** flies, bounces and rolls. It rolls fastest on a green, then on a fairway or tee, slowly in the rough, and hardly at all in a bunker. It stops where the slope cannot carry it on. In open water it is gone.
- **The ball camera.** After a drive, the view leaves you once the follow-through is done. It follows the ball from behind and a little above, until the ball has lain still for 1.5 s, and then eases back to you. To come back sooner, move or start another swing (`client/src/render/ballcam.ts`).
- **Finding your ball.** The ball you last struck is a white dot on the minimap and the paper map. When it is off the map's edge, the dot sits on the edge in the direction of the ball. It stays until the ball is holed or lost.
- **Holing out.** A ball rolling slowly over the cup drops in. Everyone is told who holed it and, for a ball played from that hole's tee, in how many strokes against the par where OSM has one: *"Ada holed out on 7 in 2 strokes (par 3): a birdie"*. Strokes are counted on the ball. Whoever hits someone else's ball starts their own count, of no hole.
- **Balls.** A tee with no ball near it gets a new one every 2 s. A struck ball lasts 5 minutes after its last stroke. At most 32 are out at once.

## The course

`osm-import` fetches `golf=*` ways and `golf=pin` nodes in a query of its own (`crates/osm-import/src/osm_golf.rs`). The result is cached like the main query, and a failed fetch leaves the world without a course rather than failing the import.

- **Surfaces.** `golf=fairway`, `driving_range`, `green`, `tee` and `bunker` become the covers `COVER_GOLF_FAIRWAY`, `GREEN`, `TEE` and `BUNKER` (14–17). They are painted over the park and kept clear of trees, rocks and bushes. An orienteering map does not cut them away.
- **Holes.** Each `golf=hole` line is played from where it starts to a cup on the green it ends on (or the nearest green within 40 m). The cup is at the green's `golf=pin` if it has one, else in the green's middle. Holes take their `ref` and `par`. Holes without a `ref` take the lowest free numbers, so their numbering is OSM's order, not the course's.
- **Flags.** Each cup gets a flagstick, `TREE_FLAG`, which is never solid.
- **`golf.json`.** The holes (tee and cup) and the surfaces' outlines, in world metres, are written beside `world.msgpack`. The server reads it at start for the cups, the tees and how the ball rolls. Holes outside the world's tiles are left out. `scripts/deploy.sh` ships it with the world (`WORLD_DIR=worlds/klappudden`).

## Tuning

The numbers are `GOLF_*` constants in `crates/shared/src/consts.rs`. Run `npm run gen:consts` in `client` after changing one, and `UPDATE_PARITY=1 cargo test -p shared` after changing the meter. They cover:
- **The ball:** its gravity (half the game's, which is twice the earth's), its bounce and skid, and the rolling friction per surface.
- **The clubs:** each club's speed and loft, and the reach.
- **The meter,** in input frames: the charge, the overswing, the marker's return, and the pure, curve and miss windows. Also the mishit's power, how hard spin bends a ball (`GOLF_SPIN`) and the putter's pull.
- **The cup:** its radius and drop speed.

The tee and strike details (`TEE_EVERY`, `REACH_ANGLE`, `SPRAY`, `START_LINE`, `TOPPED`) are in `crates/server/src/mechanics/golf.rs`. The animation's timing is in `client/src/render/golfclub.ts`.

## How it works

- **The meter.** `shared::golf::Meter` is a small state machine (idle, charging, returning, resting) stepped once per input frame, and its `Strike` holds power, curve and mishit.
  - The server steps it in `Hands::golf`, only on a frame with a new `seq`.
  - The client steps its mirror (`client/src/sim/golf.ts`) on each frame it sends.
  - So the bar on screen is exactly what the server scores, whatever the latency. A frame the jitter buffer repeats for a late one (fire stripped) cannot let go of a swing.
  - `parity.spec.ts` replays the server's sample swings (`golfMeter` in `tables.generated.json`) through the mirror.
  - Changing the held item, dying or getting in a seat resets it.
- **Server.** A ball is a projectile, `PROJECTILE_GOLF_BALL` (11), with `Rolls` (`step_rolling` in `projectiles.rs`) and a `Card` (the hole it was set on and its strokes).
  - `Combat`'s item use steps the meter and calls `golf::swing` with the strike.
  - A curved drive gets a `Spin`: in the air it pulls the ball sideways in proportion to its speed, and it is spent on the ground.
  - The `Golf` mechanic lays balls on tees and holes them out.
- **Wire.** Nothing of the meter is sent.
  - A swing is a shot event of `WEAPON_GOLF` (31) at the club head, and a ball dropping in is `WEAPON_CUP` (32).
  - A holed ball is announced as a `Notice` of `NOTICE_SAY`.
  - Protocol 69.
- **Animation.** The character model has a `golf_swing` clip (`golf_swing` in `tools/character/build_character.py`), shared by every stance. It runs from the address, through the backswing to the top (phase 0.4), impact (0.56) and the finish (0.75, held to 0.86), and back to the address. Both arms are reached onto the grip, and the right hand is laid along the shaft, its +Z towards the face. The client hangs the club on that hand as it is (`render/golfclub.ts`). Standing still with a club plays the address. Charging scrubs the backswing by the power, and the marker running back brings the club a little way down. The strike plays the rest: on the local meter's strike for the local player, and on `WEAPON_GOLF` for anyone else. Their backswing is not sent, so it is played quickly first. Rebuild the model with `Blender -b -P tools/character/build_character.py`; `--preview DIR --only golf` renders the swing face on and down the line.
