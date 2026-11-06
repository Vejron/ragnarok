<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, watch } from 'vue'
import type { HudStats } from './hud'
import type { InputController } from './sim/input'
import { winter } from './render/season'
import { touchButtons, type TouchButton } from './touchbuttons'

const props = defineProps<{ input: InputController; seat: HudStats['seat']; canSwitch?: boolean; ammo?: boolean; voice?: boolean }>()

/** How far the thumb has to travel for a full push, in CSS pixels. */
const STICK_RADIUS = 56
/** Past this fraction of a full push on foot, the soldier runs. */
const RUN_AT = 0.9
/** Touch drags are a little quicker than a mouse, which has a whole desk to cross. */
const LOOK_GAIN = 1.4

const stick = reactive({ active: false, x: 0, y: 0, dx: 0, dy: 0 })
let stickPointer = -1
let lookPointer = -1
let lookX = 0
let lookY = 0

function stickDown(event: PointerEvent) {
  if (stickPointer !== -1) return
  stickPointer = event.pointerId
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  stick.active = true
  stick.x = event.clientX
  stick.y = event.clientY
  stick.dx = stick.dy = 0
}

function stickMove(event: PointerEvent) {
  if (event.pointerId !== stickPointer) return
  let dx = event.clientX - stick.x
  let dy = event.clientY - stick.y
  const length = Math.hypot(dx, dy)
  if (length > STICK_RADIUS) {
    dx = (dx * STICK_RADIUS) / length
    dy = (dy * STICK_RADIUS) / length
  }
  stick.dx = dx
  stick.dy = dy
  const x = dx / STICK_RADIUS
  const y = -dy / STICK_RADIUS
  // In a vehicle the stick steers and Run means something else (a helicopter's descent).
  props.input.setStick(x, y, props.seat === null && Math.hypot(x, y) >= RUN_AT)
}

function stickUp(event: PointerEvent) {
  if (event.pointerId !== stickPointer) return
  stickPointer = -1
  stick.active = false
  props.input.setStick(0, 0, false)
}

function lookDown(event: PointerEvent) {
  if (lookPointer !== -1) return
  lookPointer = event.pointerId
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  lookX = event.clientX
  lookY = event.clientY
}

function lookMove(event: PointerEvent) {
  if (event.pointerId !== lookPointer) return
  props.input.lookBy((event.clientX - lookX) * LOOK_GAIN, (event.clientY - lookY) * LOOK_GAIN)
  lookX = event.clientX
  lookY = event.clientY
}

function lookUp(event: PointerEvent) {
  if (event.pointerId === lookPointer) lookPointer = -1
}

const buttons = computed(() => touchButtons(props.seat, { canSwitch: props.canSwitch ?? false, winter, ammo: props.ammo ?? false }))

/** The buttons under a finger, by pointer, so a lift (or the button going away) lets go of the right one. */
const pressed = reactive(new Map<number, TouchButton>())
/** The toggles' states, read off the input each frame: Z or a jump also changes them. */
const toggled = reactive({ prone: false, holster: false, map: false })
let frame = 0
const poll = () => {
  toggled.prone = props.input.prone
  toggled.holster = props.input.holstered
  toggled.map = props.input.mapUp
  frame = requestAnimationFrame(poll)
}
frame = requestAnimationFrame(poll)

const isPressed = (button: TouchButton) => [...pressed.values()].some((b) => b.id === button.id)
const isOn = (button: TouchButton) => (button.id === 'prone' ? toggled.prone : button.id === 'holster' ? toggled.holster : false)

function down(button: TouchButton, event: PointerEvent) {
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  pressed.set(event.pointerId, button)
  navigator.vibrate?.(10)
  if (button.mode === 'hold') props.input.hold(button.mask, true)
  else if (button.id === 'holster') props.input.holster()
  else props.input.tap(button.mask)
}

function up(event: PointerEvent) {
  const button = pressed.get(event.pointerId)
  if (!button) return
  pressed.delete(event.pointerId)
  if (button.mode === 'hold') props.input.hold(button.mask, false)
}

/** Lets go of everything: a seat's buttons are not the last one's, and a finger on one that went away never lifts from it. */
function releaseAll() {
  pressed.clear()
  stickPointer = -1
  stick.active = false
  props.input.releaseTouch()
}
watch(() => props.seat, releaseAll)
onBeforeUnmount(() => {
  cancelAnimationFrame(frame)
  releaseAll()
})

function talk(on: boolean) {
  props.input.touchTalk = on
}
</script>

<template>
  <div class="touch">
    <div class="zone left" @pointerdown="stickDown" @pointermove="stickMove" @pointerup="stickUp" @pointercancel="stickUp" />
    <div class="zone right" @pointerdown="lookDown" @pointermove="lookMove" @pointerup="lookUp" @pointercancel="lookUp" />
    <div v-if="stick.active" class="base" :style="{ left: stick.x + 'px', top: stick.y + 'px' }">
      <div class="knob" :class="{ run: seat === null && Math.hypot(stick.dx, stick.dy) >= STICK_RADIUS * RUN_AT }" :style="{ transform: `translate(${stick.dx}px, ${stick.dy}px)` }" />
    </div>
    <div class="grid">
      <button
        v-for="button in buttons"
        :key="button.id"
        type="button"
        class="btn"
        :class="[button.tone, { big: button.big, pressed: isPressed(button), on: isOn(button), small: button.label.length > 6 }]"
        :style="{ '--col': button.col, '--row': button.row }"
        :aria-pressed="button.mode === 'toggle' ? isOn(button) : undefined"
        @pointerdown.prevent="down(button, $event)"
        @pointerup="up"
        @pointercancel="up"
      >{{ button.label }}</button>
    </div>
    <div class="top">
      <button type="button" class="chip" :class="{ on: toggled.map }" @pointerdown.prevent="input.mapUp = !input.mapUp">MAP</button>
      <button v-if="voice" type="button" class="chip" @pointerdown.prevent="talk(true)" @pointerup="talk(false)" @pointercancel="talk(false)">TALK</button>
    </div>
    <div class="rotate">Turn your device sideways to play</div>
  </div>
</template>

<style scoped>
.touch {
  --btn: 64px; --gap: 10px; --edge-r: max(18px, env(safe-area-inset-right)); --edge-b: max(20px, env(safe-area-inset-bottom));
  position: fixed; inset: 0; pointer-events: none; user-select: none; -webkit-user-select: none; touch-action: none;
}
.zone { position: absolute; top: 0; bottom: 0; pointer-events: auto; touch-action: none; }
.zone.left { left: 0; width: 42%; top: 25%; }
.zone.right { right: 0; width: 58%; }
.base {
  position: absolute; width: 112px; height: 112px; margin: -56px 0 0 -56px; border-radius: 50%;
  background: rgba(255, 255, 255, 0.3); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.knob {
  position: absolute; left: 50%; top: 50%; width: 52px; height: 52px; margin: -26px 0 0 -26px; border-radius: 50%;
  background: rgba(255, 255, 255, 0.85); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3); transition: background 0.1s;
}
/* Pushed far enough to run. */
.knob.run { background: rgba(143, 209, 106, 0.95); }
.btn {
  position: absolute; width: var(--btn); height: var(--btn); border-radius: 50%; border: 0; padding: 0; pointer-events: auto; touch-action: none;
  right: calc(var(--edge-r) + var(--col) * (var(--btn) + var(--gap)));
  bottom: calc(var(--edge-b) + var(--row) * (var(--btn) + var(--gap)));
  font: 800 12px ui-rounded, 'SF Pro Rounded', system-ui, sans-serif; color: #1d2b36; background: rgba(255, 255, 255, 0.65);
  box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3); -webkit-tap-highlight-color: transparent;
  transition: transform 0.06s, background 0.06s;
}
.btn.small { font-size: 11px; line-height: 1.1; }
/* Centred on its cell, bigger than the rest. */
.btn.big { width: 80px; height: 80px; margin: 0 -8px -8px 0; font-size: 14px; }
.btn.fire { background: rgba(255, 90, 95, 0.75); }
.btn.reload { background: rgba(255, 190, 11, 0.75); }
.btn.on { box-shadow: 0 0 0 3px #2ec4b6, 0 0 0 5px rgba(0, 0, 0, 0.3); background: rgba(255, 255, 255, 0.9); }
.btn.pressed { transform: scale(0.92); background: rgba(255, 255, 255, 0.95); }
.btn.fire.pressed { background: rgba(255, 90, 95, 1); }
.btn.reload.pressed { background: rgba(255, 190, 11, 1); }
.top { position: absolute; top: max(12px, env(safe-area-inset-top)); right: calc(var(--edge-r) + 172px); display: flex; gap: 8px; }
.chip {
  pointer-events: auto; touch-action: none; min-width: 56px; height: 36px; padding: 0 12px; border: 0; border-radius: 999px;
  font: 800 12px ui-rounded, 'SF Pro Rounded', system-ui, sans-serif; color: #1d2b36; background: rgba(255, 255, 255, 0.75);
  box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25); -webkit-tap-highlight-color: transparent;
}
.chip:active, .chip.on { background: rgba(255, 255, 255, 0.95); box-shadow: 0 0 0 3px #2ec4b6; }
.rotate { display: none; }
@media (orientation: portrait) {
  .rotate {
    display: grid; place-content: center; position: absolute; inset: 0; pointer-events: auto; padding: 24px; text-align: center;
    background: rgba(20, 40, 60, 0.85); color: white; font: 800 22px ui-rounded, 'SF Pro Rounded', system-ui, sans-serif;
  }
}
</style>
