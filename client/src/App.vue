<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { Game, type HudStats } from './game'

const container = ref<HTMLElement | null>(null)
const stats = shallowRef<HudStats | null>(null)
const error = ref<string | null>(null)
let game: Game | null = null
let lastHud = 0

onMounted(() => {
  game = new Game(container.value!, (next) => {
    // The HUD only needs a few updates a second.
    const now = performance.now()
    if (now - lastHud < 200 && next.locked === stats.value?.locked) return
    lastHud = now
    stats.value = next
  })
  game.start().catch((reason: unknown) => {
    error.value = reason instanceof Error ? reason.message : String(reason)
  })
})

onBeforeUnmount(() => game?.dispose())
</script>

<template>
  <div ref="container" class="viewport" />
  <div v-if="stats" class="hud">
    <span :class="['status', stats.status.toLowerCase()]">{{ stats.status === 'CONNECTED' ? 'ONLINE' : stats.status === 'CONNECTING' ? 'CONNECTING' : 'SOLO' }}</span>
    <span v-if="stats.rtt !== null">{{ Math.round(stats.rtt) }} ms</span>
    <span>{{ stats.players }} {{ stats.players === 1 ? 'player' : 'players' }}</span>
    <span class="debug">interp {{ Math.round(stats.interpDelayMs) }} ms · fix {{ stats.correction.toFixed(2) }} m · unacked {{ stats.pending }}</span>
  </div>
  <div v-if="stats?.prompt" class="prompt">{{ stats.prompt }}</div>
  <div v-if="stats?.seat === 'driver'" class="drive-hint">W/S gas and brake · A/D steer or turn · Space handbrake</div>
  <div v-if="stats?.seat === 'gunner'" class="drive-hint">Mouse aim · Click or F fire · {{ stats.reload < 1 ? 'reloading…' : 'loaded' }}</div>
  <div v-if="stats?.seat === 'gunner'" class="reticle" :class="{ loading: stats.reload < 1 }" />
  <div v-if="stats?.warning" class="banner">{{ stats.warning }}</div>
  <div v-if="error" class="banner">{{ error }}</div>
  <div v-if="stats && !stats.locked" class="overlay">
    <h1>RAGNAROK</h1>
    <p>Click to play</p>
    <p class="keys">WASD move · Shift run · Space jump · E drive, ride or man a gun · Mouse look and aim · Click fire · Esc release</p>
  </div>
</template>

<style>
html, body, #app { margin: 0; height: 100%; overflow: hidden; background: #9fd3f0; }
body { font-family: ui-rounded, 'SF Pro Rounded', system-ui, sans-serif; }
</style>

<style scoped>
.viewport { position: fixed; inset: 0; }
.hud {
  position: fixed; top: 12px; left: 12px; display: flex; flex-wrap: wrap; gap: 8px;
  font-size: 13px; font-weight: 700; color: #1d2b36; pointer-events: none;
}
.hud > span { background: rgba(255, 255, 255, 0.8); padding: 4px 10px; border-radius: 999px; }
.status.connected { color: #138a4b; }
.status.connecting { color: #b26b00; }
.status.offline { color: #b3261e; }
.debug { font-weight: 500; opacity: 0.8; }
.banner {
  position: fixed; top: 52px; left: 50%; transform: translateX(-50%); max-width: calc(100% - 32px);
  background: #ffe066; color: #1d2b36; padding: 8px 14px; border-radius: 10px; font-weight: 700;
}
.prompt {
  position: fixed; bottom: 18%; left: 50%; transform: translateX(-50%); pointer-events: none;
  background: rgba(255, 255, 255, 0.9); color: #1d2b36; padding: 8px 18px; border-radius: 999px; font-weight: 800;
}
.drive-hint {
  position: fixed; bottom: 12px; left: 50%; transform: translateX(-50%); pointer-events: none;
  font-size: 13px; font-weight: 700; color: #1d2b36; background: rgba(255, 255, 255, 0.8); padding: 4px 12px; border-radius: 999px;
}
.reticle {
  position: fixed; top: 50%; left: 50%; width: 26px; height: 26px; margin: -13px 0 0 -13px; pointer-events: none;
  border: 3px solid rgba(255, 255, 255, 0.95); border-radius: 50%; box-shadow: 0 0 0 1.5px rgba(0, 0, 0, 0.45);
}
.reticle.loading { border-color: rgba(255, 190, 120, 0.9); border-style: dashed; }
.overlay {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center;
  background: rgba(20, 40, 60, 0.35); color: white; pointer-events: none;
  text-shadow: 0 2px 0 rgba(0, 0, 0, 0.3);
}
.overlay h1 { margin: 0; font-size: clamp(40px, 10vw, 88px); letter-spacing: 0.08em; transform: rotate(-3deg); }
.overlay p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.overlay .keys { font-size: 14px; font-weight: 500; opacity: 0.9; }
</style>
