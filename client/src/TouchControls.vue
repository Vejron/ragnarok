<script setup lang="ts">
import { reactive } from 'vue'
import type { InputController } from './sim/input'
import { BUTTON_CROUCH, BUTTON_FIRE, BUTTON_JUMP, BUTTON_RELOAD, BUTTON_RUN, BUTTON_USE } from './sim/consts'

const props = defineProps<{ input: InputController; seat: string | null; canSwitch?: boolean }>()

/** How far the thumb has to travel for a full push, in CSS pixels. */
const STICK_RADIUS = 56
/** Past this fraction of a full push on foot, the stickman runs. */
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

const press = (mask: number) => (event: PointerEvent) => {
  ;(event.currentTarget as HTMLElement).setPointerCapture(event.pointerId)
  props.input.hold(mask, true)
}
const release = (mask: number) => () => props.input.hold(mask, false)
</script>

<template>
  <div class="touch">
    <div class="zone left" @pointerdown="stickDown" @pointermove="stickMove" @pointerup="stickUp" @pointercancel="stickUp" />
    <div class="zone right" @pointerdown="lookDown" @pointermove="lookMove" @pointerup="lookUp" @pointercancel="lookUp" />
    <div v-if="stick.active" class="base" :style="{ left: stick.x + 'px', top: stick.y + 'px' }">
      <div class="knob" :style="{ transform: `translate(${stick.dx}px, ${stick.dy}px)` }" />
    </div>
    <button class="btn use" @pointerdown.prevent="input.tap(BUTTON_USE)">USE</button>
    <button class="btn jump" @pointerdown.prevent="press(BUTTON_JUMP)($event)" @pointerup="release(BUTTON_JUMP)()" @pointercancel="release(BUTTON_JUMP)()">
      {{ seat === 'driver' ? 'BRAKE' : seat === 'aviator' ? 'FASTER' : seat === 'pilot' ? 'UP' : 'JUMP' }}
    </button>
    <button v-if="seat === 'pilot' || seat === 'aviator'" class="btn sink" @pointerdown.prevent="press(BUTTON_RUN)($event)" @pointerup="release(BUTTON_RUN)()" @pointercancel="release(BUTTON_RUN)()">{{ seat === 'aviator' ? 'SLOWER' : 'DOWN' }}</button>
    <button v-if="!seat" class="btn crouch" @pointerdown.prevent="press(BUTTON_CROUCH)($event)" @pointerup="release(BUTTON_CROUCH)()" @pointercancel="release(BUTTON_CROUCH)()">CROUCH</button>
    <button v-if="seat === 'aviator'" class="btn crouch" @pointerdown.prevent="press(BUTTON_CROUCH)($event)" @pointerup="release(BUTTON_CROUCH)()" @pointercancel="release(BUTTON_CROUCH)()">BOMB</button>
    <button class="btn fire" @pointerdown.prevent="press(BUTTON_FIRE)($event)" @pointerup="release(BUTTON_FIRE)()" @pointercancel="release(BUTTON_FIRE)()">FIRE</button>
    <button v-if="!seat" class="btn reload" @pointerdown.prevent="input.tap(BUTTON_RELOAD)">RELOAD</button>
    <button v-else-if="canSwitch" class="btn reload" @pointerdown.prevent="input.tap(BUTTON_RELOAD)">SEAT</button>
  </div>
</template>

<style scoped>
.touch { position: fixed; inset: 0; pointer-events: none; user-select: none; -webkit-user-select: none; touch-action: none; }
.zone { position: absolute; top: 0; bottom: 0; pointer-events: auto; touch-action: none; }
.zone.left { left: 0; width: 42%; top: 25%; }
.zone.right { right: 0; width: 58%; }
.base {
  position: absolute; width: 112px; height: 112px; margin: -56px 0 0 -56px; border-radius: 50%;
  background: rgba(255, 255, 255, 0.3); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.knob {
  position: absolute; left: 50%; top: 50%; width: 52px; height: 52px; margin: -26px 0 0 -26px; border-radius: 50%;
  background: rgba(255, 255, 255, 0.85); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3);
}
.btn {
  position: absolute; width: 68px; height: 68px; border-radius: 50%; border: 0; pointer-events: auto; touch-action: none;
  font: 800 12px ui-rounded, system-ui, sans-serif; color: #1d2b36; background: rgba(255, 255, 255, 0.6);
  box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.3); -webkit-tap-highlight-color: transparent;
}
.btn:active { background: rgba(255, 255, 255, 0.9); }
.fire { right: max(18px, env(safe-area-inset-right)); bottom: max(24px, env(safe-area-inset-bottom)); width: 84px; height: 84px; background: rgba(255, 90, 95, 0.7); }
.reload { right: calc(max(18px, env(safe-area-inset-right)) + 96px); bottom: max(24px, env(safe-area-inset-bottom)); background: rgba(255, 190, 11, 0.7); font-size: 10px; }
.jump { right: max(26px, env(safe-area-inset-right)); bottom: calc(max(24px, env(safe-area-inset-bottom)) + 100px); }
.sink { right: calc(max(26px, env(safe-area-inset-right)) + 80px); bottom: calc(max(24px, env(safe-area-inset-bottom)) + 100px); }
.crouch { right: calc(max(26px, env(safe-area-inset-right)) + 80px); bottom: calc(max(24px, env(safe-area-inset-bottom)) + 100px); font-size: 10px; }
.use { right: max(26px, env(safe-area-inset-right)); bottom: calc(max(24px, env(safe-area-inset-bottom)) + 180px); }
</style>
