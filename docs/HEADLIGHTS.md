# Headlights

Every vehicle that can be driven has lamps, and its driver switches them with **L** (in a headset, the left hand's X button).

## What it does

- **The driver switches them.** A press of L in the driver's seat (seat 0) turns the vehicle's lamps on or off; holding it switches them once. It is not the driver's own headlamp, which only L on foot or in another seat switches ([WINTER.md](WINTER.md)).
- **They go out by themselves** when the driver's seat is left empty or the vehicle is wrecked. A car's also go out while its front lamps are smashed (`LIGHTS_BREAK`), until it is repaired.
- **They are for the night.** At full strength once the sun is down (`nightAt`), and 15% of it by day, so lamps switched on at noon are still seen to be on. They fade in and out over a quarter second.
- **Where they shine from**, by kind:
  - A car (jeep, sports car, Roadster, technical, golf cart, bus, snowmobile, snow racer): from the middle of the `lamp` meshes on its nose (`CarModel.headlight`), dipped a little toward the road, 40 m.
  - A helicopter: the searchlight under its nose gun, turning and nodding with the gun (`HeliModel.searchlight`), 80 m in a narrower beam.
  - An air ambulance: the same beam, fixed under its nose and dipped ahead (`AirAmbulanceModel.searchlight`).
  - A Titan: the searchlight in its visor, the way its head faces, so it lights what the pilot looks at (`TitanModel.searchlight`), cold white, 320 m ([VEHICLES-XIV.md](VEHICLES-XIV.md)).
  - Anything else (tanks, the Pbv and Strf, planes, ships, the submarine, drones, the walker): from the middle of the front of its hull a little above its centre, along the nose, as a car's (`Headlights.hullLamp`).
- **What has none**: the towed guns (mortar, Pak), the ship's guns, missiles and loitering munitions in flight, and the hen.

## How it works

- **Server.** `Vehicle::lamp` is the switch. The `Vehicles` tick reads the driver's `BUTTON_LAMP` with the rest of their controls, switching on a fresh press (`Vehicle::lamp_held`), and clears it when seat 0 is empty or the vehicle has no health left. `Movement` leaves a driver's own headlamp alone. A Titan's searchlight is the same switch: it has no lamp state of its own.
- **Wire.** A vehicle sends a `flags` component while its lamps are on, holding `FLAG_LAMP` (bit 0, the soldiers' grounded bit), and none while they are off ([NETWORKING.md](NETWORKING.md), protocol 83).
- **Client.** The `Headlights` presenter (`entities/presenters/headlights.ts`) holds one spot light per lit vehicle through `PointLights`, from where its model says, and the beams pass draws its cone in mist and rain ([RENDERING.md](RENDERING.md)).
