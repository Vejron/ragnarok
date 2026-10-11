<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { Game, type HudStats } from './game'
import Hotbar from './Hotbar.vue'
import Minimap from './Minimap.vue'
import TouchControls from './TouchControls.vue'
import type { InputController } from './sim/input'
import { colorFor } from './render/character'
import type { Look } from './render/look'
import { WEAPON_AUTOCANNON, WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_ROCKET, WEAPON_SHELL, WEAPON_WRECK } from './net/codec'

const container = ref<HTMLElement | null>(null)
const stats = shallowRef<HudStats | null>(null)
const error = ref<string | null>(null)
const input = shallowRef<InputController | null>(null)
const look = ref<Look>({ soft: true, contact: true, ao: true, far: true })
let game: Game | null = null
/** M: the minimap grows to the whole map. */
const mapOpen = ref(false)
const onKey = (event: KeyboardEvent) => {
  if (event.code === 'KeyM' && !event.repeat) mapOpen.value = !mapOpen.value
}
let lastHud = 0

onMounted(() => {
  window.addEventListener('keydown', onKey)
  game = new Game(container.value!, (next) => {
    // The HUD only needs a few updates a second.
    const now = performance.now()
    if (now - lastHud < 200 && next.locked === stats.value?.locked) return
    lastHud = now
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
  { key: 'ao', label: 'Ambient occlusion' },
  { key: 'far', label: 'Far shadows' },
]
function toggleLook(key: keyof Look) {
  look.value = { ...look.value, [key]: !look.value[key] }
  game?.setLook(look.value)
}

onBeforeUnmount(() => {
  window.removeEventListener('keydown', onKey)
  game?.dispose()
})

const swatch = (id: number) => `#${colorFor(id).toString(16).padStart(6, '0')}`
const who = (id: number) => (id === 0 ? 'the fall' : `Player ${id}`)
/** What the kill feed shows between killer and victim. */
const mark = (weapon: number) =>
  weapon === WEAPON_SHELL || weapon === WEAPON_AUTOCANNON || weapon === WEAPON_ROCKET || weapon === WEAPON_GRENADE || weapon === WEAPON_WRECK ? '💥' : weapon === WEAPON_REVOLVER ? '›‹' : '›'
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
  <div v-if="stats?.prompt" class="prompt">{{ stats.touch ? stats.prompt.replace(/^E:/, 'USE:') : stats.prompt }}</div>
  <TouchControls v-if="stats?.touch && stats.locked && input" :input="input" :seat="stats.seat" :can-switch="stats.canSwitchSeat" />
  <template v-if="!stats?.touch">
  <div v-if="stats?.seat === 'driver'" class="drive-hint">W/S gas and brake · A/D steer or turn · Space handbrake<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'pilot'" class="drive-hint">W/S lean forward and back · A/D turn · Space climb · Shift sink<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'gunner'" class="drive-hint">Mouse aim · Click or F fire · {{ stats.reload < 1 ? 'reloading…' : 'loaded' }}<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  </template>
  <div v-if="stats?.seat === 'gunner'" class="reticle" :class="{ loading: stats.reload < 1 }" />
  <div v-else-if="stats?.scoped" class="scope"><i class="scope-h" /><i class="scope-v" /></div>
  <div v-else-if="stats?.armed" class="cross" :class="{ aim: stats.aiming }"><i /><i /><i /><i /></div>
  <div v-if="stats?.onFoot" class="health" :class="{ low: stats.hp <= stats.maxHp / 4 }">
    <div class="health-fill" :style="{ width: (100 * stats.hp) / stats.maxHp + '%' }" />
    <span>{{ stats.hp }}</span>
  </div>
  <Hotbar
    v-if="stats?.onFoot"
    :slots="stats.hotbar"
    :selected="stats.selected"
    :progress="stats.progress"
    :touch="stats.touch"
    @select="(slot) => input?.selectSlot(slot)"
  />
  <Minimap v-if="stats && stats.halfExtent > 0 && !stats.touch" :x="stats.x" :z="stats.z" :yaw="stats.yaw" :half-extent="stats.halfExtent" :open="mapOpen" />
  <div v-if="stats?.dead" class="death">
    <h2>ELIMINATED</h2>
    <p>Respawning in {{ Math.ceil(stats.respawnIn) }}</p>
  </div>
  <ul v-if="stats?.feed.length" class="feed">
    <li v-for="(entry, i) in stats.feed" :key="i" :class="entry.you ?? ''">
      <template v-if="entry.killer !== 0">
        <i :style="{ background: swatch(entry.killer) }" />{{ who(entry.killer) }}
        <b>{{ mark(entry.weapon) }}</b>
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
  <div v-if="stats && !stats.locked" class="look" @click.stop>
    <label v-for="option in LOOKS" :key="option.key">
      <input type="checkbox" :checked="look[option.key]" @change="toggleLook(option.key)" />{{ option.label }}
    </label>
  </div>
  <div v-if="stats && !stats.locked" class="overlay">
    <h1>RAGNAROK</h1>
    <p>{{ stats.touch ? 'Tap to play' : 'Click to play' }}</p>
    <p v-if="stats.touch" class="keys">Left thumb moves · Drag the right side to look · Buttons jump, crouch, fire, reload and use · Tap the slots to switch items</p>
    <p v-else class="keys">WASD move · Shift run · Space jump · E take, drive, ride or man a gun · 1-5 or wheel pick an item · Mouse look · Click fire · Right mouse aim (scope with the sniper) · R reload · M map · Tab scores · Esc release</p>
  </div>
</template>

<style>
html, body, #app { margin: 0; height: 100%; overflow: hidden; background: #9fd3f0; touch-action: none; overscroll-behavior: none; }
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
.cross { position: fixed; top: 50%; left: 50%; width: 0; height: 0; pointer-events: none; --gap: 7px; --len: 6px; }
.cross.aim { --gap: 3px; }
.cross i { position: absolute; background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 0.5px rgba(0, 0, 0, 0.3); transition: transform 0.12s; }
.cross i:nth-child(1), .cross i:nth-child(2) { width: 2px; height: var(--len); margin-left: -1px; }
.cross i:nth-child(1) { top: calc(var(--gap) * -1 - var(--len)); }
.cross i:nth-child(2) { top: var(--gap); }
.cross i:nth-child(3), .cross i:nth-child(4) { height: 2px; width: var(--len); margin-top: -1px; }
.cross i:nth-child(3) { left: calc(var(--gap) * -1 - var(--len)); }
.cross i:nth-child(4) { left: var(--gap); }
.scope {
  position: fixed; inset: 0; pointer-events: none;
  background: radial-gradient(circle at center, transparent 0, transparent 38vmin, #000 38.4vmin);
}
.scope i { position: absolute; background: rgba(0, 0, 0, 0.85); }
.scope-h { top: 50%; left: calc(50% - 38vmin); width: 76vmin; height: 1px; }
.scope-v { left: 50%; top: calc(50% - 38vmin); height: 76vmin; width: 1px; }
.reticle.loading { border-color: rgba(255, 190, 120, 0.9); border-style: dashed; }
.health {
  position: fixed; left: 16px; bottom: 16px; width: 220px; height: 22px; border-radius: 999px; overflow: hidden; pointer-events: none;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.health-fill { height: 100%; background: #2ec4b6; transition: width 0.15s; }
.health.low .health-fill { background: #ff5a5f; }
.health span { position: absolute; inset: 0; display: grid; place-content: center; font-size: 13px; font-weight: 800; color: #1d2b36; }
.death {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center; pointer-events: none;
  background: rgba(150, 20, 20, 0.35); color: white; text-shadow: 0 2px 0 rgba(0, 0, 0, 0.35);
}
.death h2 { margin: 0; font-size: clamp(32px, 8vw, 72px); letter-spacing: 0.08em; }
.death p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.feed {
  position: fixed; top: 206px; right: 12px; margin: 0; padding: 0; list-style: none; pointer-events: none;
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
</style>
