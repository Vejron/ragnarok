<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { Game, type HudStats } from './game'
import { hotbarRectsCss } from './hud/gauges'
import TouchControls from './TouchControls.vue'
import type { InputController } from './sim/input'
import { LOOK_OPTIONS, type Look } from './render/look'
import { QUALITIES, resolveQuality, type Quality } from './render/quality'
import { applyUpdate, updateReady } from './pwa'

// The HUD itself is drawn by the game (`hud/`); the page keeps what is clicked or tapped, and banners.
const container = ref<HTMLElement | null>(null)
const stats = shallowRef<HudStats | null>(null)
const error = ref<string | null>(null)
const input = shallowRef<InputController | null>(null)
const look = ref<Look>({ soft: true, contact: true, far: true, ink: true, bloom: true, reflect: true, grass: true, scorch: true, adaptive: true, netStats: false })
const lookOpen = ref(false)
const quality = ref<Quality>('auto')
let game: Game | null = null
let lastStats = 0
const viewport = ref({ width: window.innerWidth, height: window.innerHeight })
const onResize = () => (viewport.value = { width: window.innerWidth, height: window.innerHeight })

onMounted(() => {
  window.addEventListener('resize', onResize)
  game = new Game(container.value!, (next) => {
    // The page only needs a few updates a second.
    const now = performance.now()
    if (now - lastStats < 200 && next.locked === stats.value?.locked && next.hands === stats.value?.hands) return
    lastStats = now
    stats.value = next
  })
  input.value = game.input
  look.value = { ...game.look }
  quality.value = game.quality
  game.start().catch((reason: unknown) => {
    error.value = reason instanceof Error ? reason.message : String(reason)
  })
})

function chooseQuality(next: Quality) {
  quality.value = next
  game?.setQuality(next)
}

function toggleLook(key: keyof Look) {
  look.value = { ...look.value, [key]: !look.value[key] }
  game?.setLook(look.value)
}

onBeforeUnmount(() => {
  window.removeEventListener('resize', onResize)
  game?.dispose()
})

const settingsUrl = `${import.meta.env.BASE_URL}settings.html`
/** On a touch screen, where the drawn hotbar's slots are, to be tapped. */
const slotTaps = computed(() => (stats.value?.touch && stats.value.hands ? hotbarRectsCss(viewport.value.width, viewport.value.height, true) : []))

/** The keys, in pairs, for the pause screen. */
const KEYS: [string, string][] = [
  ['WASD', 'move'], ['Shift', 'run'], ['Space', 'jump'], ['C', 'crouch'], ['Z', 'lie down'],
  ['E', 'take, drive, ride or man a gun'], ['1–5 / wheel', 'pick an item'], ['Click', 'fire'],
  ['Right mouse', 'aim (scope with the sniper or Carl Gustaf)'], ['R', 'reload'], ['X', 'holster'],
  ['G', 'parachute'], ['K', 'skis (winter)'], ['L', 'headlamp'], ['V', 'talk'], ['M', 'read the map'],
  ['Tab', 'scores'], ['F3', 'network stats'], ['Esc', 'release the mouse'],
]
</script>

<template>
  <div ref="container" class="viewport" />
  <TouchControls
    v-if="stats?.touch && stats.locked && input"
    :input="input"
    :seat="stats.seat"
    :can-switch="stats.canSwitchSeat"
    :ammo="stats.ammo !== null"
    :voice="stats.status === 'CONNECTED' && stats.voice.mic !== 'unavailable'"
  />
  <button
    v-for="(slot, k) in slotTaps"
    :key="k"
    type="button"
    class="slot-tap"
    :style="{ left: slot.x + 'px', top: slot.y + 'px', width: slot.w + 'px', height: slot.h + 'px' }"
    :aria-label="`Slot ${k + 1}`"
    @pointerdown.prevent="input?.selectSlot(k)"
  />
  <!-- The game's own warnings are drawn on the HUD, under the pills; this is for a game that would not start. -->
  <div v-if="error" class="banner">{{ error }}</div>
  <!-- Offered rather than done, so nobody is pulled out of a fight; on the pause screen it is one tap away. -->
  <button v-if="updateReady && !stats?.locked" type="button" class="chip update" @click.stop="applyUpdate">New version · Reload</button>
  <div v-if="stats && !stats.locked" class="overlay">
    <h1>RAGNAROK</h1>
    <p class="play">{{ stats.touch ? 'Tap to play' : 'Click to play' }}</p>
    <div class="actions" @click.stop>
      <a class="chip" :href="settingsUrl" title="Leaves the match: you join again when you come back">Settings · name, colour, loadout, spawns</a>
      <button type="button" class="chip" :aria-expanded="lookOpen" @click="lookOpen = !lookOpen">Graphics {{ lookOpen ? '▴' : '▾' }}</button>
    </div>
    <div v-if="lookOpen" class="look" @click.stop>
      <div class="quality" role="radiogroup" aria-label="Quality">
        <span>Quality</span>
        <button
          v-for="option in QUALITIES"
          :key="option.key"
          type="button"
          role="radio"
          :aria-checked="quality === option.key"
          :class="{ on: quality === option.key }"
          :title="option.key === 'auto' ? `${resolveQuality('auto')} on this device` : undefined"
          @click="chooseQuality(option.key)"
        >{{ option.label }}</button>
      </div>
      <label v-for="option in LOOK_OPTIONS" :key="option.key" :title="option.about">
        <input type="checkbox" :checked="look[option.key]" @change="toggleLook(option.key)" />{{ option.label }}
      </label>
    </div>
    <p v-if="stats.touch" class="keys">Left thumb moves · Drag the right side to look · Hold AIM to aim · Tap the slots to switch items</p>
    <dl v-else class="keys">
      <div v-for="[key, what] in KEYS" :key="key"><dt>{{ key }}</dt><dd>{{ what }}</dd></div>
    </dl>
  </div>
</template>

<style>
html, body, #app { margin: 0; height: 100%; overflow: hidden; background: #9fd3f0; touch-action: none; overscroll-behavior: none; }
body { font-family: ui-rounded, 'SF Pro Rounded', system-ui, sans-serif; }
</style>

<style scoped>
.viewport { position: fixed; inset: 0; }
.banner {
  position: fixed; top: 52px; left: 50%; transform: translateX(-50%); max-width: calc(100% - 32px);
  background: #ffe066; color: #1d2b36; padding: 8px 14px; border-radius: 10px; font-weight: 700;
}
/* The drawn hotbar's slots, to tap on a touch screen. */
.slot-tap {
  position: fixed; padding: 0; border: 0; background: transparent; touch-action: none; -webkit-tap-highlight-color: transparent;
}
.overlay {
  position: fixed; inset: 0; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 10px;
  padding: 16px; overflow-y: auto; text-align: center;
  background: rgba(20, 40, 60, 0.45); color: white; pointer-events: none;
  text-shadow: 0 2px 0 rgba(0, 0, 0, 0.3);
}
.overlay h1 { margin: 0; font-size: clamp(40px, 10vw, 88px); letter-spacing: 0.08em; transform: rotate(-3deg); }
.overlay .play { margin: 0; font-size: 22px; font-weight: 800; }
.actions { display: flex; flex-wrap: wrap; justify-content: center; gap: 8px; pointer-events: auto; }
.chip {
  display: inline-block; padding: 8px 16px; border: 0; border-radius: 999px; font: inherit; font-size: 15px; font-weight: 700;
  background: rgba(255, 255, 255, 0.92); color: #1d2b36; text-decoration: none; text-shadow: none; cursor: pointer;
}
.chip:hover { background: white; }
.update { position: fixed; top: calc(12px + env(safe-area-inset-top)); right: 16px; z-index: 1; background: #ffe066; }
.update:hover { background: #ffeb94; }
.look {
  pointer-events: auto; display: grid; grid-template-columns: repeat(auto-fill, minmax(170px, 1fr)); gap: 8px 16px;
  width: min(560px, 100%); padding: 12px 16px; border-radius: 14px; text-align: left; text-shadow: none;
  background: rgba(255, 255, 255, 0.94); color: #1d2b36; font-size: 14px; font-weight: 700;
}
.look label { display: flex; align-items: center; gap: 8px; cursor: pointer; }
.quality { grid-column: 1 / -1; display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.quality span { margin-right: 4px; }
.quality button {
  padding: 4px 12px; border-radius: 999px; border: 2px solid #1d2b36; background: transparent; color: inherit;
  font: inherit; cursor: pointer;
}
.quality button.on { background: #1d2b36; color: white; }
.keys {
  display: grid; grid-template-columns: repeat(auto-fill, minmax(230px, 1fr)); gap: 4px 20px;
  width: min(760px, 100%); margin: 8px 0 0; font-size: 14px; font-weight: 500; text-align: left;
}
.keys div { display: flex; gap: 8px; align-items: baseline; }
.keys dt { flex: none; min-width: 92px; text-align: right; font-weight: 800; }
.keys dd { margin: 0; opacity: 0.92; }
p.keys { display: block; text-align: center; }
</style>
