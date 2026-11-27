# The phone

The soldier carries a phone. Raised, it is held in both hands in front of the eyes, as the paper map used to be. Its screen has three apps:
- **Map:** the world's map, from tiles sharp down to the paving stones.
- **Deliveries:** the board of food orders ([DELIVERY.md](DELIVERY.md)).
- **Dealer:** the vehicles a dealer sells and what they cost, bought from the delivery cash at a dealer's carport ([DEALERS.md](DEALERS.md)).

## Using it

- **M** brings it up on the Map, **T** on the Deliveries. The same key again puts it away, and the other key switches app. **E** by a dealer's carport, with nothing else to hand, brings it up on the Dealer app. The tabs along the bottom of the screen switch app too.
- While it is up the mouse is free, as holding Alt is in some games. WASD, run, jump and crouch still work, so you can walk while you read. Nothing fires, because both hands are on the phone.
- **Click on the screen** to tap. **Drag** pans the map or scrolls the list, and **the wheel** over the screen zooms the map or scrolls the list. The map's **+** and **−** zoom, and **◎** makes the map follow you again after a drag.
- **Click off the phone** to put it away and take the mouse back. **Esc** puts it away and pauses.
- It is used on foot or from a passenger's seat, alive. Taking a driver's or gunner's seat, or dying, puts it away.
- On a touch screen the **PHONE** chip brings it up on the Map, and a tap on the screen is a tap on the phone.
- The view goes first person while it is up, as it did for the paper map. Other players see nothing different.

## The map

- The Map app and the minimap draw the importer's map tiles (`maptiles/`, [WORLD.md](WORLD.md#map-tiles)): each fetches the few squares it shows, at the level its zoom needs.
- The map opens on about 800 m across. Its zoom runs from about 8 km to 130 m across, in ten steps of about 1.6 times.
- It shows what the minimap does:
  - your arrow;
  - the blips;
  - the backpack's circle;
  - the delivery pin, with a pip on the edge when off the screen.
- It also has a scale bar, north, and along the top the line of the delivery in hand.

## How it works

- **The model.** `HeldPhoneView` (`client/src/render/heldphone.ts`) is a dark slab in two gloved hands, as the paper sheet was: `held` meshes in a group that follows the camera ([RENDERING.md](RENDERING.md)).
  - It is built at life size and drawn 2.6 times as large and as far off. That is the same picture, but kept beyond the camera's near plane of 0.25 m, which a phone at reading distance is not.
  - The screen is a 540 × 1080 canvas texture, unlit, because a screen lights itself.
- **Redrawing.**
  - The screen is redrawn when something changed: an app, a tap, a pan or zoom, or a map tile arriving.
  - It is redrawn every frame during a drag. Otherwise it is redrawn every `heldPhoneMs` (125 ms, or 250 ms on low quality), so the marks move.
  - With the phone down it costs nothing.
  - Up and redrawn every frame, it added 0.08 ms to a 2.3 ms Retina frame (`gpu-bench`, the forest view).
- **The screen.**
  - `PhoneScreen` (`client/src/phone/screen.ts`) draws the status bar, the app in front and the tabs.
  - Each draw returns the places a tap lands (`Region`). A press and release within 14 px is a tap on the region under it; more is a drag.
  - The apps are `MapApp` (`phone/mapapp.ts`) and `JobsApp` (`phone/jobsapp.ts`). The jobs app draws on the canvas what the delivery phone used to show in a Vue panel.
- **Tapping.**
  - Each frame, `HeldPhoneView.follow` projects the screen's four corners onto the page.
  - `PhonePointer` (`phone/pointer.ts`) turns a pointer on the page into the screen's own pixels by the inverse of the projective map those corners make (`phone/homography.ts`), and hands it to `PhoneScreen`.
  - With the phone up, a click on the canvas does not take the pointer lock (`InputController`).
- **Input.** `InputController.phone` is the app shown, or null. `togglePhone(app)` is M and T, `putPhoneAway()` is a click off it, and `dropPhone()` is dying or a seat that cannot use it. With the phone up, `locked` stays true, so the game neither pauses nor stops reading the keys.
- **Tests.**
  - `phone.spec.ts`: the keys, the pointer and that nothing fires.
  - `phonescreen.spec.ts`: the corners' map, tabs, taking and giving back an order, buying at a dealer, panning, zooming and following again.
  - `maptiles.spec.ts`: picking the level, the coarser tile while one loads, a square with none, and the cache's limit.
