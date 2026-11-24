# Vehicles XXIV: boost for everything that drives, and dust only when it is earned

Only the snowmobile had a boost on run (Shift). Now every car, truck, tank and boat has one: holding run with the throttle open makes the engine pull much harder for a few seconds, which gets a car off the line, up a slope or out of a corner far quicker. Off the boost a vehicle now tops out at 90% of its old top speed; on it, it can reach its old top speed and a little more (105%). The tyres' dust was also thrown up too freely, even off a car standing still; now it only comes off tyres that slide in a hard turn, lock under braking or spin pulling away.

## The boost

- **Who has it.** Every powered car (the jeep, the sports car, the golf cart, the technical, the period cars), the bus, the AMV and the EPA traktor, the tracked vehicles (the tank, the Strv S, the Strf 90 and the Pbv 302, afloat too), the CB90 and the raft, and the snowmobile as before. The Roadster keeps its own thrusters on run; the snowracer has no engine; the ship sails on the wind and the submarine dives on run; walkers, hens, the Titan and aircraft keep what they had.
- **Controls.** Run (Shift, or BOOST on a touch screen) boosts, jump is the handbrake (a boat's anchor): `Buttons::Boost`, which the snowmobile's old `Buttons::Sled` became.
- **How long.** It is the snowmobile's: full (`BOOST_FULL`) lasts about four seconds held with the throttle open (`BOOST_DRAIN`) and fills again in about six once let go (`BOOST_REFILL`). Held without the throttle it is neither spent nor refilled, so it does not flicker on and off. The HUD shows it as a BOOST gauge on the driver's seat (`'boost'` in a seat's `gauges`).
- **How much.** `Spec::boost` is a `Boost { power, cruise, speed }`: what the engine's pull (or a hull's thrust) is multiplied by on the boost, and the shares of `Spec::max_speed` it tops out at off it (`cruise`) and on it (`speed`). `Spec::cruise_speed()` is the top speed off the boost; the traffic cruises at a share of that.

| `Boost` | Power | Top off it | Top on it | Kinds |
|---|---|---|---|---|
| `Boost::CAR` | ×1.8 | 90% | 105% | cars, the golf cart, the technical |
| `Boost::HEAVY` | ×1.5 | 90% | 105% | bus, AMV, the tracked vehicles |
| `Boost::GOVERNED` | ×1.5 | 100% | 100% | the EPA traktor, held to 30 km/h by law either way |
| `Boost::BOAT` | ×1.6 | 90% | 105% | CB90, raft |
| `SNOWMOBILE_BOOST` | ×1.8 | 100% | 135% | the snowmobile, unchanged |

The boost lasts about four seconds, which is not enough for most cars to reach the higher top speed from a standstill: it is a ceiling, reached from speed or downhill. Some things now want it: the Strv S and the Strf 90 charge a 1.1 m wall on the boost, and the Bronco rolls at full lock at the speed it only makes on it.

A second after setting off flat out, the jeep makes 13 m/s on the boost against 7.4 m/s without, the golf cart 8.8 against 4.9 and the tank 10.2 against 7.2 (`every_driven_car_and_tank_boosts_off_the_line_but_tops_out_about_the_same`).

The boost lives in the vehicle's `tank`, which the snapshot already carries as `Thrust { air, flags }` (`THRUST_BOOST` while it is open), and the client predicts it with the rest of the vehicle; nothing new goes over the wire. Wheeled vehicles boost in `drive_car` (and skimming or swimming in `wade`), boats in `boat`. Bots and the traffic never hold run.

## Dust

The effects (`render/tyres.ts`) worked out how hard each tyre slid from how far its contact point moved sideways between two frames. The drawn position jitters by a centimetre or so as snapshots and corrections come in, and over a 120th of a second that reads as more than a metre a second sideways: a parked car "slid", and threw dust, grit and skid marks. Now:

- A tyre's sideways speed comes from the vehicle's **smoothed** motion (`speed`'s smoothing) and its smoothed turning rate, not from one frame's difference.
- The acceleration the tyres go by is the real one, the change of the smoothed speed (`surge` in `tyres.ts`). The old reading divided the smoothed speed's lag by one frame, which made it about five times too high at 60 fps and ten at 120: a car coasting down at 2 m/s² read as braking at 10 and "locked" its wheels. The engine's sound, the exhaust and the snow plumes are tuned to the old reading and still use it.
- A launch spins the wheels from 8.5 m/s² (`SPIN_ACCEL`), and only once the vehicle is moving (`SPIN_MOVING`, 0.5 m/s, in `sim/slip.ts`): a jeep or a Volvo floored throws none, the Manta floored and anything on the boost does. Braking hard (about 11 m/s²) still locks them.
- Dust and grit come off the tyres only from that slip: sliding in a hard turn, locking, or spinning on a hard launch. The trail of dust behind any quick car off-road is gone; it still ruts soft ground. Mud off a marsh, spray in water and the snowmobile's snow plume are as they were.
