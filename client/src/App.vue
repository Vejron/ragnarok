<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { Game, type HudStats } from './game'
import { hotbarRects } from './hud/gauges'
import TouchControls from './TouchControls.vue'
import type { InputController } from './sim/input'
import type { Look } from './render/look'

// The HUD itself is drawn by the game (`hud/`); the page keeps what is clicked or tapped, and banners.
const container = ref<HTMLElement | null>(null)
const stats = shallowRef<HudStats | null>(null)
const error = ref<string | null>(null)
const input = shallowRef<InputController | null>(null)
const look = ref<Look>({ soft: true, contact: true, far: true, ink: true, bloom: true, reflect: true, grass: true, scorch: true })
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
  game.start().catch((reason: unknown) => {
    error.value = reason instanceof Error ? reason.message : String(reason)
  })
})

const LOOKS: { key: keyof Look; label: string }[] = [
  { key: 'soft', label: 'Soft light' },
  { key: 'contact', label: 'Contact shadows' },
  { key: 'far', label: 'Far shadows' },
  { key: 'ink', label: 'Ink outlines' },
  { key: 'bloom', label: 'Bloom' },
  { key: 'reflect', label: 'Water reflections' },
  { key: 'grass', label: 'Grass' },
  { key: 'scorch', label: 'Scorched craters' },
]
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
const slotTaps = computed(() => (stats.value?.touch && stats.value.hands ? hotbarRects(viewport.value.width, viewport.value.height, true) : []))
</script>

<template>
  <div ref="container" class="viewport" />
  <TouchControls v-if="stats?.touch && stats.locked && input" :input="input" :seat="stats.seat" :can-switch="stats.canSwitchSeat" />
  <button
    v-for="(slot, k) in slotTaps"
    :key="k"
    type="button"
    class="slot-tap"
    :style="{ left: slot.x + 'px', top: slot.y + 'px', width: slot.w + 'px', height: slot.h + 'px' }"
    :aria-label="`Slot ${k + 1}`"
    @pointerdown.prevent="input?.selectSlot(k)"
  />
  <div v-if="stats?.warning" class="banner">{{ stats.warning }}</div>
  <div v-if="error" class="banner">{{ error }}</div>
  <div v-if="stats && !stats.locked" class="look" @click.stop>
    <label v-for="option in LOOKS" :key="option.key">
      <input type="checkbox" :checked="look[option.key]" @change="toggleLook(option.key)" />{{ option.label }}
    </label>
  </div>
  <div v-if="stats && !stats.locked" class="overlay">
    <h1>RAGNAROK</h1>
    <p>{{ stats.touch ? 'Tap to play' : 'Click to play' }}</p>
    <p><a class="settings" :href="settingsUrl" @click.stop>Name, colour and spawn points</a></p>
    <p v-if="stats.touch" class="keys">Left thumb moves · Drag the right side to look · Buttons jump, crouch, fire, reload and use · Tap the slots to switch items</p>
    <p v-else class="keys">WASD move · Shift run · Space jump · E take, drive, ride or man a gun · 1-5 or wheel pick an item · Mouse look · Click fire · Right mouse aim (scope with the sniper or Carl Gustaf) · R reload · X holster · K skis (winter) · L headlamp · V talk · M read the map · Tab scores · Esc release</p>
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
.look {
  position: fixed; right: 12px; bottom: 12px; z-index: 1; display: flex; flex-direction: column; gap: 6px;
  padding: 10px 14px; border-radius: 14px; background: rgba(255, 255, 255, 0.92); color: #1d2b36; font-size: 13px; font-weight: 700;
}
.look label { display: flex; align-items: center; gap: 8px; cursor: pointer; }
.overlay {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center;
  background: rgba(20, 40, 60, 0.35); color: white; pointer-events: none;
  text-shadow: 0 2px 0 rgba(0, 0, 0, 0.3);
}
.overlay h1 { margin: 0; font-size: clamp(40px, 10vw, 88px); letter-spacing: 0.08em; transform: rotate(-3deg); }
.overlay p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.overlay .keys { font-size: 14px; font-weight: 500; opacity: 0.9; }
.overlay .settings {
  pointer-events: auto; display: inline-block; padding: 6px 16px; border-radius: 999px; font-size: 15px;
  background: rgba(255, 255, 255, 0.92); color: #1d2b36; text-decoration: none; text-shadow: none;
}
.overlay .settings:hover { background: white; }
</style>
