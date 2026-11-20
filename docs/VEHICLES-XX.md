# Vehicles XX: the raft

The raft is a small inflatable river-rafting boat for two with a big outboard on its transom: 3.6 m long, 1.6 m across its tubes, and light enough that the motor has it out of the water and on the plane in two seconds. It floats a hand's breadth deep, so it runs where the river is too thin for the Stridsbåt 90, and it is made for running Vindelälven's rapids.

## In play

- **The raft** (`KIND_RAFT` = 35) has 60 hp. Its tubes are rubber: rounds go straight through, into whoever sits behind them.
- **Placement.** It appears only where it is placed in the map editor (**Raft**, D in the palette), and only on open water: placed on land or on ice, it is skipped. Vindeln has two, side by side in the calm water about 110 m above the rapid at x −500, z −1871, pointing downstream.
- **It floats** 12 cm deep (`RAFT_DRAFT`) and rides the river's waves and standing waves, as every boat does. The current carries it: let go, it drifts downstream.
- **Seat 0 is the driver,** sitting on the left tube at the stern with a hand on the tiller.
  - W drives it ahead: it is on the plane by 7 m/s within two seconds and tops out at 14 m/s (27 knots).
  - S, while it runs ahead, puts the outboard astern and stops it at 4.5 m/s²; from a stop it backs at up to 3 m/s. Space does the same.
  - A/D turn it by swinging the outboard's thrust: up to 1.9 rad/s under way on the throttle, a circle about 15 m across flat out, and 1.4 rad/s standing still. It snaps into a turn: its rate of turn follows the helm at 7 a second (`RAFT_HELM`; the CB90's 3). Let go of W and it answers the helm only lazily (`RAFT_THRUST_STEER`: 0.65 of its turning comes from the motor's push). That gets it round a quarter turn in under half the time the Stridsbåt 90 takes.
  - Its light bow rears up as it climbs onto the plane (0.16 rad) and settles a little once it is on it. It banks into its turns, up to 0.22 rad.
  - 40 cm of water is plenty: there the Stridsbåt 90 is aground and the raft runs at full speed. In water thinner than its draft it scrapes on over the bed with half its drive (see below).
- **Seat 1 is the rider** on the thwart in the bow, who fires their own gun, as a car's passenger does. The driver cannot: their hand is on the tiller.
- **The crew are in the open** (`Exposure::Open`).
- **The outboard** is on the transom and its fuel tank on the floor in front of it. Either can be shot out.

## In the rapids

A first drive said it handled like a car: the current carried it as a conveyor belt would, and nothing in the water ever turned it. A rubber boat in a rapid is thrown about. Each of these is a field of `Boat`, so the Stridsbåt 90 keeps its old handling (its values are in brackets):

- **The water turns it** (`lever` 2, CB90 0.3). `water_under` (`crates/sim/src/vehicle/water.rs`) now feels the current at the bow, the stern and both sides as well as the middle. Where they differ, the water under the hull turns (`Water::turn`, rad/s): half its vorticity, from the difference along the hull and the difference across it. The hull's yaw rate is eased towards the helm's plus `lever` times that.
  - Let go astride an eddy line, with the river going up one side at 3 m/s and the eddy coming back down the other at 1 m/s, the raft comes round about 0.9 rad in three seconds and the CB90 hardly at all.
  - The current that carries it is now the average over its bow, stern and middle, not the middle's alone.
- **It carves** (`keel` 3.5 a second, CB90 2.5). Hard over on the throttle it whips round, 1.5 rad in a second (coasting, 0.65), with a short slide at the start of the turn: at the most, 0.46 of its way goes sideways. A first tuning let it slide far more (`keel` 0.8, then 1.2, up to 0.87 sideways), and it spun round while still going the old way, like a log.
- **It sits down when let off** (`coast` 3 m/s², CB90 0.8). With `grip`, let go flat out it stops in 1.2 s and about 6 m.
- **It pitches and rolls over every wave** (`level` 40 a second², `rock` 2.5 a second; CB90 10 and 3). It is brought level with the water under it four times as hard as the CB90 and its rocking dies away a little slower, so it follows each wave and bobs. On the open swell it tips about 0.12 rad to the CB90's 0.08.
- **The river takes it up** (`grip` 1.5 a second, CB90 0). Coasting, the water takes its way through the water out quickly, on top of the usual coasting drag. Let go into a current of 3 m/s, it is going with it at over 2.5 m/s within a second; the CB90's way through the water holds it back to under 1.5 m/s.
- **It answers quickly** (`helm` 7 a second, CB90 3): its rate of turn follows the helm and the water's turning that fast.
- **It is thrown off crests** (`heave` 10 a second, CB90 3). The face of a wave lifts the light raft at once, so it comes over the crest still rising and flies off the back. On a crest of 0.6 m it leaves the water by about 8 cm and the CB90 not at all. The water also no longer pulls a hull down after a surface falling away from it: its hold goes with how deep the hull still sits in it. Coming down onto the water faster than 2 m/s, it slams and loses way (6 m/s² for each m/s more).
- **It scrapes over the shallows** (`scrape` 0.5, CB90 0). In water too thin to float it, it keeps half its drive and its grip on the water. Its hull has a low friction (0.1, as a tracked hull's belly), and its collider is now an outline (`RAFT_PROFILE`) whose floor sweeps up into the tubes at the bow and transom. A flat box caught dead on the seam between two blocks of terrain; the swept floor rides up over it, or over a ledge or a rock, and is knocked aside. Over a gravel bar 2 cm deep, it scrapes across and the CB90 runs aground. `RAFT_BOW`, the taper of its old box, is gone.

## What changed to make it

- **`Lift::Boat` carries its boat's tuning:** `Lift::Boat(&'static Boat)`, with the new `Boat` in `crates/shared/src/vehicles/spec.rs`. It holds its draft, thrust, braking, top speed astern, turn rate standing still, planing, bow rise and bank. `crates/sim/src/vehicle/boat.rs` reads them from there, where it read the `CB90_*` constants before. The Stridsbåt 90's are its `CB90_HULL`, from the same constants, so it is driven exactly as it was. `Lift` is no longer `Eq`, as `Boat` holds floats.
- **The raft's tuning** is `RAFT_HULL` in `crates/shared/src/vehicles/raft.rs`, from the `RAFT_*` constants. Its `Body` takes 0.4 of a car's crash damage, rams with 0.3 of a car's weight and takes knocks from below up to 8 m/s, so a rock in a rapid does not tear it.

## The client

- **`client/src/kinds/raft.ts`** is the kind. The driver's seat is the plain chase camera, the rider's a passenger's. Its outboard is a new synth, `OUTBOARD` (`kinds/engines.ts`): a three-cylinder two-stroke snarl on the `marine` engine model, climbing as it gets up on the plane. It floats (`fx.floats`), so it carves a wake into the wave field. Its drive is `fx.drive: 'outboard'`: a white wash behind its transom in the water (`VehicleFx.wash`, [WATER-WAVES.md](WATER-WAVES.md)), and a prop wash in the air (`JetSpray.outboard`). That is not the CB90's tall column of streaks, which did not look like water off a propeller, but a low fan thrown back and out to either side of the motor: big drops that arc and drop back, fine ones and a thin mist. It grows with its speed through the water (fullest at 12 m/s) and bursts up, to 2.5 times as much and higher, as the hull slaps down off a wave (coming down at 2.5 m/s or more), so in a rapid it comes in bursts.
- **`client/src/render/raft.ts`** is `RaftModel`, drawn in code as the pirate ship is, with no `.glb`. It is about 1,200 triangles:
  - its tubes are one tube swept round a path from the left stern round the bow to the right stern, with the bow kicking up 14 cm. They come in red, yellow, blue or grey by the raft's id;
  - a dark rubbing strake runs low round them, and a grab line along each side;
  - there is a floor, a transom board, a thwart across the bow, a red fuel tank and two paddles stowed along the right tube;
  - the outboard is a cowling, a leg, an anti-cavitation plate, a gearcase and a three-bladed propeller, with a tiller reaching forward to the driver.
- **The outboard swings with the helm.** Neither the helm nor the motor's angle is on the wire, and a boat's pose reports no steering, so the model reads the helm from how fast the raft is turning and swings the motor on its bracket, up to 0.5 rad (`RAFT_MOTOR_SWING`). Its propeller spins while someone has the tiller, faster the faster it goes. Since the water turns the raft too, the motor also swings as an eddy spins it.

## Tests

- `crates/sim/src/vehicle/boat.rs`:
  - `a_raft_floats_a_hands_breadth_deep_and_its_outboard_has_it_on_the_plane_in_two_seconds`
  - `a_raft_runs_through_water_too_thin_to_float_the_combat_boat` (40 cm of water)
  - `a_raft_turns_inside_the_combat_boat`
  - `let_go_on_an_eddy_line_a_raft_is_spun_round_and_the_combat_boat_barely`
  - `let_go_into_running_water_a_raft_is_taken_up_by_it_within_a_second`
  - `a_raft_coasting_answers_its_helm_lazily_and_under_power_whips_round_sliding`
  - `a_raft_is_thrown_off_a_crest_the_combat_boat_rides_over`
  - `a_raft_scrapes_over_a_shoal_the_combat_boat_runs_aground_on`
  - `a_raft_rocks_over_the_swell_and_sits_down_in_the_water_when_let_off`

  The CB90's tests are unchanged and pass on the shared boat.
- `crates/shared/src/vehicles.rs`: the raft is the one kind besides the tracked ones with an outline.
- `client/src/__tests__/tyres.spec.ts`: the raft's prop wash is a fan, far less than the CB90's rooster tail, and more as it bobs over waves. `waterwaves.spec.ts`: the CB90 and the raft leave a white wash behind their transoms.
- `client/src/__tests__/raft.spec.ts`: the model stays under 2,500 triangles; it swings its outboard the way it turns and holds it straight otherwise; its propeller spins only while someone has the tiller; the kind's seats, crew guns and exposure.

## Protocol

`PROTOCOL_VERSION` is unchanged: the only new thing on the wire is the kind id, `KIND_RAFT` (35).
