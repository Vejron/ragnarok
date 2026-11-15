# VR: piloting a Titan from a headset

The game can be shown in a VR headset, and a Titan ([VEHICLES-XIV.md](VEHICLES-XIV.md)) piloted from its head with the pilot's own hands moving its fists.

**Not yet run on a headset.** It was built on a Mac, which has no way to show it. What is checked: the hand mapping and the projection, by tests; and the whole frame drawn once for each of two eyes into one texture, in the real engine in headless Chrome. The session with a headset itself has never run. Expect to fix things the first time it does.

## What it needs

- **A browser with WebXR on WebGPU.** The engine is WebGPU only, so the ordinary WebXR path (WebGL) is no use to it. It needs the WebXR/WebGPU binding (`XRGPUBinding`), which is a draft:
  - Chrome on Windows has it behind flags. In `chrome://flags`, search for WebXR and turn on the ones for projection layers and for the WebGPU binding, then restart. Their names have changed between versions, and I have not seen them: check against the Chrome you have. The game shows **Enter VR** only once `XRGPUBinding` exists, which is the test that they are on.
  - A headset the PC's OpenXR runtime drives: a Quest over Link or Air Link, or a SteamVR headset.
- Where the binding is missing the game is as it was: no **Enter VR** button is shown.

## Using it

1. Open the game in that Chrome, with the headset connected.
2. On the start screen, press **Enter VR**. The picture moves to the headset.
3. Walk to a Titan and board it. With no keyboard in reach, do that before putting the headset on, or have somebody else place you: boarding is still E.
4. In the Titan:

| You | The Titan |
|---|---|
| Move a controller | That fist moves the same way from its shoulder, 40 times as far |
| Squeeze a grip | That hand closes: on a soldier or a vehicle it is on, or into a fist that punches |
| Let go of the grip | It opens: what it held is put down, or thrown if the arm was swinging |
| Left thumbstick | Forward and back walk; left and right turn |
| Right thumbstick | Left and right turn too |
| Duck your head 25 cm | It crouches, which brings its fists to the ground |
| A button, right hand | It leaps; held in the air, its foot thrusters fire while the fuel lasts |
| Right index trigger | The autocannon in its right fist fires, along its forearm |
| Left index trigger | A missile goes up from its left shoulder, and you fly it as the submarine's officer does; the Titan stands meanwhile |

- Its arms follow yours at their own pace: they swing at 45 m/s at most, so a fast jab of yours becomes a long heavy punch.
- A fist moving at 8 m/s or more hits only if it is closed. An open hand does no harm, so reach for a soldier open-handed and then squeeze.
- The right hand holds the gun, so squeezing it never picks anything up: it only makes a fist to punch with.

## How it is put together

- **The session** (`client/src/xr/headset.ts`) asks for `immersive-vr` with the `webgpu` feature, makes an `XRGPUBinding` on the renderer's own device and one projection layer in the canvas's format, and reads each frame: an eye's pose, lens and part of the layer's texture for each view, both controllers' grip poses and squeeze buttons, and both thumbsticks. The types are declared in that file, since none are published for the binding.
- **The device** has to come from an adapter asked for with `xrCompatible`. `Renderer.create` asks for one when the page has `XRGPUBinding`, and takes any adapter when there is none that will do.
- **The frames** come from the session's `requestAnimationFrame` while it lasts (`Game.nextFrame`), since the page's own stops.
- **Drawing** (`Scene.render`, `Renderer.renderEye`): the whole frame is drawn once for each eye, at the eye's size, into its part of the headset's texture.
  - The second eye keeps the first's shadow map and the first's exposure, so both eyes are lit alike.
  - A headset's lens is not symmetric about the eye. `PerspectiveCamera.lens` takes its projection, turned to the engine's reversed depth (`reversedDepth`): a session on WebGPU gives depth 0 at the near plane and 1 at the far.
  - The last pass reads its pixels from the start of the eye's part of the texture (`Output.clip.zw`).
- **The cockpit** is the Titan's head: the camera is put at `TITAN_EYE`, facing the way the Titan does, and the headset's space is laid over it turned to that heading and nothing else. The Titan leans as it walks and when it crouches; the cockpit stays level, for the pilot's stomach.
- **The hands** (`client/src/xr/hands.ts`): each controller is measured from where the pilot's shoulder is taken to be (19 cm out from the eyes and 22 cm down), and that reach, 40 times over (the Titan's arm of 24 m against a pilot's of 0.6 m), is asked of the fist from the Titan's shoulder, in its hull's frame. It goes to the server as the same `hands` message the mouse and keyboard send, so the server knows nothing of VR.
- **The cost.** Everything is drawn twice. Measured on the screen's path, the frame is the same as before this was added (forest 3.5 ms and water 3.0 ms at 2268×1258 on the M-series laptop, old and new alternated three times with `gpu-bench.mjs`). What two eyes at a headset's size cost on the PC is not known yet: start with Graphics on Low.

## Picking it up

Where to start reading, and what to do first.

- **The code, in the order a frame goes through it:**
  - `client/src/App.vue`: the **Enter VR** chip, shown when `Game.vrSupported()` says so.
  - `client/src/game.ts`: `enterVr` and `leaveVr` start and end the session; `nextFrame` takes frames from it; `readHeadset` turns the thumbsticks and a ducked head (`VR_DUCK`) into the ordinary input; `moveFists` sends the hands.
  - `client/src/xr/headset.ts`: everything that touches WebXR. Nothing else in the client names an XR type.
  - `client/src/xr/hands.ts`: the arithmetic, with no browser in it (`fistsFromHands`, `reversedDepth`), tested in `client/src/__tests__/titan.spec.ts`.
  - `client/src/render/scene.ts`: `see(eyes)`, `worn`, and `render`, which places the camera for each eye.
  - `engine/src/renderer.ts`: `renderEye` and `EyeTarget` ([RENDERING.md](RENDERING.md), "A headset's two eyes").
  - `client/src/cameras.ts`: the `titan` rig puts the camera in the head when `scene.worn`.
- **The server has no VR in it.** A headset's pilot sends the same `hands` message as the mouse does (`crates/server/src/mechanics/titans.rs`), so anything about what fists do is tested there without a headset.
- **Checking the drawing without a headset.** Run the dev page in headless Chrome (`--headless=new --remote-debugging-port=9333 --enable-unsafe-webgpu --use-angle=metal`, the page on port 5287), then:
  ```sh
  node client/scripts/cdp.mjs eval "$(cat client/scripts/vr-eyes.js)"
  node client/scripts/cdp.mjs shot eyes.png
  ```
  It draws two eyes from one spot into one texture and reports validation errors and the share of pixels that differ, which should be none and 0. Give the eyes different `position`s in the script to see the two pictures part.
- **The first run on a headset.** What is most likely to be wrong, in the order to look:
  1. *No **Enter VR** button*: `XRGPUBinding` is not in the page (the flags), or `isSessionSupported('immersive-vr')` said no (the runtime).
  2. *`requestSession` or `createProjectionLayer` throws*: the binding is a draft (the immersive-web group's WebXR-WebGPU-Binding), and `headset.ts` was written in November 2026 from what was published then, without a browser to try it in. Compare its local type declarations with what Chrome has now (the layer's options, `getViewSubImage`, the sub-image's `colorTexture`, `viewport` and `getViewDescriptor`).
  3. *A black or a doubled picture*: how the layer's texture holds the two eyes. `headset.ts` takes both the view descriptor (an array's layer) and the viewport (a place in a wide texture) from each sub-image, and `EyeTarget.x/y` carry the second; look at which the headset really gives.
  4. *The world inside out, or nothing but sky*: the depth range of the projection (`reversedDepth` takes it to be 0..1).
  5. *The fists going the wrong way*: the signs and the heading in `fistsFromHands`; the test beside it says what is meant.
  6. *Too slow*: Graphics on Low, then a preset of its own (reflections, shafts and flare off first).
- **What to build next**, roughly by worth: predicting the pilot's own fists in the WASM mirror (they lag a round trip, which a headset will make plain); a way to board without the keyboard; the HUD on a panel in the cockpit; calibrating the arm's length; a graphics preset.

## Not yet

- **It has not run on a headset** (above).
- **Only a Titan is meant for it.** On foot and in other vehicles the picture is in the headset, from where the screen's camera would be, but nothing is made for the controllers there: no boarding, firing or walking by hand.
- **The HUD is fixed to the head**, drawn over each eye as on the screen, not on a panel in the cockpit. The page's own overlays (the start screen, the scoreboard, prompts) are not in the headset at all.
- **The shoulders are guessed** from the head, with no calibration, and the pilot is taken to face the way the cockpit does.
- **No picture on the screen** while the headset is on.
- **The first frame after the picture changes size can differ between the eyes** (about 8 % of the pixels a little brighter in the first eye, now and then, in the headless check). Every frame after it, the two eyes drawn from one spot are the same to the pixel. A session sets the size once, as it starts.
- **The graphics are not turned down** for it by themselves.
