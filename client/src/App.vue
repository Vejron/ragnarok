<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { Game, type HudStats } from './game'
import { colorFor } from './render/stickman'
import { WEAPON_SHELL } from './net/codec'

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

const swatch = (id: number) => `#${colorFor(id).toString(16).padStart(6, '0')}`
const who = (id: number) => (id === 0 ? 'the fall' : `Player ${id}`)
</script>

<template>
  <div ref="container" class="viewport" />
  <div v-if="stats" class="hud">
    <span :class="['status', stats.status.toLowerCase()]">{{ stats.status === 'CONNECTED' ? 'ONLINE' : stats.status === 'CONNECTING' ? 'CONNECTING' : 'SOLO' }}</span>
    <span v-if="stats.rtt !== null">{{ Math.round(stats.rtt) }} ms</span>
    <span>{{ stats.players }} {{ stats.players === 1 ? 'player' : 'players' }} in view</span>
    <span v-if="stats.status === 'CONNECTED'">{{ stats.kills }} kills · {{ stats.deaths }} deaths</span>
    <span class="debug">tiles {{ stats.tiles }} · interp {{ Math.round(stats.interpDelayMs) }} ms · fix {{ stats.correction.toFixed(2) }} m · unacked {{ stats.pending }}</span>
  </div>
  <div v-if="stats?.prompt" class="prompt">{{ stats.prompt }}</div>
  <div v-if="stats?.seat === 'driver'" class="drive-hint">W/S gas and brake · A/D steer or turn · Space handbrake</div>
  <div v-if="stats?.seat === 'pilot'" class="drive-hint">W/S lean forward and back · A/D turn · Space climb · Shift sink</div>
  <div v-if="stats?.seat === 'gunner'" class="drive-hint">Mouse aim · Click or F fire · {{ stats.reload < 1 ? 'reloading…' : 'loaded' }}</div>
  <div v-if="stats?.seat === 'gunner'" class="reticle" :class="{ loading: stats.reload < 1 }" />
  <div v-else-if="stats?.armed" class="reticle foot" />
  <div v-if="stats?.armed" class="health" :class="{ low: stats.hp <= stats.maxHp / 4 }">
    <div class="health-fill" :style="{ width: (100 * stats.hp) / stats.maxHp + '%' }" />
    <span>{{ stats.hp }}</span>
  </div>
  <div v-if="stats?.armed" class="rocket" :class="{ ready: stats.rocket >= 1 }">
    <div class="rocket-fill" :style="{ width: 100 * stats.rocket + '%' }" />
    <span>ROCKET</span>
  </div>
  <div v-if="stats?.dead" class="death">
    <h2>ELIMINATED</h2>
    <p>Respawning in {{ Math.ceil(stats.respawnIn) }}</p>
  </div>
  <ul v-if="stats?.feed.length" class="feed">
    <li v-for="(entry, i) in stats.feed" :key="i" :class="entry.you ?? ''">
      <template v-if="entry.killer !== 0">
        <i :style="{ background: swatch(entry.killer) }" />{{ who(entry.killer) }}
        <b>{{ entry.weapon === WEAPON_SHELL ? '💥' : '›' }}</b>
      </template>
      <i :style="{ background: swatch(entry.victim) }" />{{ who(entry.victim) }}<template v-if="entry.killer === 0"> took {{ who(0) }}</template>
    </li>
  </ul>
  <table v-if="stats?.board.length" class="board">
    <thead><tr><th>Player</th><th>Kills</th><th>Deaths</th></tr></thead>
    <tbody>
      <tr v-for="row in stats.board" :key="row.id" :class="{ you: row.you }">
        <td><i :style="{ background: swatch(row.id) }" />{{ row.you ? 'You' : `Player ${row.id}` }}</td>
        <td>{{ row.kills }}</td>
        <td>{{ row.deaths }}</td>
      </tr>
    </tbody>
  </table>
  <div v-if="stats?.warning" class="banner">{{ stats.warning }}</div>
  <div v-if="error" class="banner">{{ error }}</div>
  <div v-if="stats && !stats.locked" class="overlay">
    <h1>RAGNAROK</h1>
    <p>Click to play</p>
    <p class="keys">WASD move · Shift run · Space jump · E drive, ride or man a gun · Mouse look and aim · Click fire · Right click rocket · Tab scores · Esc release</p>
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
.reticle.foot { width: 8px; height: 8px; margin: -4px 0 0 -4px; border-width: 2px; }
.reticle.loading { border-color: rgba(255, 190, 120, 0.9); border-style: dashed; }
.health {
  position: fixed; left: 16px; bottom: 16px; width: 220px; height: 22px; border-radius: 999px; overflow: hidden; pointer-events: none;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.health-fill { height: 100%; background: #2ec4b6; transition: width 0.15s; }
.rocket {
  position: fixed; left: 16px; bottom: 46px; width: 220px; height: 14px; border-radius: 999px; overflow: hidden; pointer-events: none;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.rocket-fill { height: 100%; background: #ffbe0b; }
.rocket.ready .rocket-fill { background: #ff8a2a; }
.rocket span { position: absolute; inset: 0; display: grid; place-content: center; font-size: 10px; font-weight: 800; color: #1d2b36; }
.health.low .health-fill { background: #ff5a5f; }
.health span { position: absolute; inset: 0; display: grid; place-content: center; font-size: 13px; font-weight: 800; color: #1d2b36; }
.death {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center; pointer-events: none;
  background: rgba(150, 20, 20, 0.35); color: white; text-shadow: 0 2px 0 rgba(0, 0, 0, 0.35);
}
.death h2 { margin: 0; font-size: clamp(32px, 8vw, 72px); letter-spacing: 0.08em; }
.death p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.feed {
  position: fixed; top: 12px; right: 12px; margin: 0; padding: 0; list-style: none; pointer-events: none;
  display: flex; flex-direction: column; align-items: flex-end; gap: 4px; font-size: 13px; font-weight: 700; color: #1d2b36;
}
.feed li { background: rgba(255, 255, 255, 0.85); padding: 4px 10px; border-radius: 999px; }
.feed li.killer { box-shadow: 0 0 0 2px #2ec4b6; }
.feed li.victim { box-shadow: 0 0 0 2px #ff5a5f; }
.feed i, .board i { display: inline-block; width: 10px; height: 10px; border-radius: 50%; margin: 0 6px 0 2px; }
.feed b { margin: 0 6px; }
.board {
  position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); pointer-events: none; min-width: 320px;
  background: rgba(255, 255, 255, 0.92); border-radius: 14px; padding: 10px 16px; color: #1d2b36; font-weight: 700; border-collapse: collapse;
}
.board th { text-align: right; font-size: 12px; opacity: 0.6; padding: 4px 8px; }
.board th:first-child, .board td:first-child { text-align: left; }
.board td { text-align: right; padding: 4px 8px; }
.board tr.you td { color: #138a4b; }
.overlay {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center;
  background: rgba(20, 40, 60, 0.35); color: white; pointer-events: none;
  text-shadow: 0 2px 0 rgba(0, 0, 0, 0.3);
}
.overlay h1 { margin: 0; font-size: clamp(40px, 10vw, 88px); letter-spacing: 0.08em; transform: rotate(-3deg); }
.overlay p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.overlay .keys { font-size: 14px; font-weight: 500; opacity: 0.9; }
</style>
