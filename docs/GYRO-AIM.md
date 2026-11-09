# Gyro aim

On a phone, one thumb moves and the other has to look, aim and fire. Gyro aim lets the player turn the view by turning the phone, the way the big mobile shooters do, so the right thumb can stay on FIRE. Dragging to look still works, and the two add together: a drag makes the big turns and the gyroscope makes the fine ones.

It is client-only. The gyroscope feeds `InputController.turnBy`, the same yaw and pitch the mouse and the touch drag change, so prediction, the server and the wire format do not know it exists.

## Modes

The player picks a mode in **Controls** on the pause screen (touch devices only) or on the settings page. It is stored in `localStorage` under `ragnarok.controls` (`client/src/controls.ts`), apart from the graphics' `Look`.

| Mode | The gyroscope turns the view |
|---|---|
| Off (the default) | never |
| While aiming | while AIM is held, to fine-tune a shot |
| Always | whenever the game is being played, on foot or in a seat |

It is off by default, because on iOS it needs a permission and because a view that moves when the player shifts their grip surprises anyone who did not ask for it. It never turns the view with the map up. Sensitivity is a multiple, 0.25× to 3×, of `GYRO_GAIN` (1.5: the view turns one and a half times as far as the phone).

## The sums (`client/src/sim/gyro.ts`)

The `devicemotion` event gives `rotationRate`, in degrees per second about the device's own axes:
- `beta` is about x, to the right in portrait.
- `gamma` is about y, up the screen in portrait.
- `alpha` is about z, out of the screen.

`gyroTurn` integrates the rate over the time since the last reading. Rates are used rather than `deviceorientation`'s absolute angles, because rates have nothing to recentre and do not jump where an angle wraps.

- **Screen axes.** `screen.orientation.angle` turns the device's axes into the screen's: with the angle θ, the screen's right is (cos θ, −sin θ, 0) and its up is (sin θ, cos θ, 0). The game is played in landscape (90 or 270), where turning the phone left is about the device's x axis, with opposite signs for the two ways round.
- **Yaw** is the turn about the world's vertical, which is taken from gravity (`accelerationIncludingGravity`, smoothed by `followGravity`). A phone held tilted back, or flat in the lap, still turns the way the player turns, and a steering-wheel roll of an upright phone does nothing. Browsers disagree on the accelerometer's sign, so the up vector is flipped to face the player's side of the phone: the screen faces the player and its top is above its bottom. Without an accelerometer the screen's up stands in.
- **Pitch** is the turn about the screen's right-hand edge, so tipping the top of the screen away raises the view.
- **Deadzone.** Rates under `GYRO_DEADZONE` (0.6°/s) are ignored. A phone held still then neither drifts nor keeps resetting the pause that stops a car's camera swinging back behind it after the player looks around.
- **Gaps.** No more than `GYRO_MAX_DT` (0.1 s) is integrated between two readings, and the first reading after the gyroscope was idle only sets the clock. A stalled frame or a hidden tab coming back does not throw the view.

`turnBy` applies the game's `lookScale`, as the mouse does, so a zoomed scope is no more twitchy under the gyroscope than under the mouse.

## Permission

iOS 13 and later hand over the motion sensors only after `DeviceMotionEvent.requestPermission()`, called from a tap. The grant lasts for that page load. Two taps ask:
- choosing a mode in the pause screen's Controls;
- the tap that starts play (the `click` in `InputController`), if a mode is set.

If the player refuses, the mode falls back to Off for the session, and the pause screen says so. Other browsers do not ask. Motion events need a secure context, which the game already is because WebTransport needs one.

## Testing

`client/src/__tests__/gyro.spec.ts` covers the axis mapping in both landscapes and in portrait, the deadzone, the flat phone, the two signs of gravity, the cap on gaps, and `turnBy`. Only a real phone checks the feel: Chrome's sensor emulation sets orientation, not `rotationRate`. Dispatch synthetic `DeviceMotionEvent`s from the console to check the wiring on a desktop.
