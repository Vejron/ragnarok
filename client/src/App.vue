<script setup lang="ts">
import { onBeforeUnmount, onMounted, ref, shallowRef, watch } from 'vue'
import { Game, type HudStats } from './game'
import Hotbar from './Hotbar.vue'
import Minimap from './Minimap.vue'
import TouchControls from './TouchControls.vue'
import type { InputController } from './sim/input'
import { colorFor, cssColor, isBot, nameFor } from './profile'
import type { Look } from './render/look'
import { LIMB_ARM_L, LIMB_ARM_R, LIMB_LEG_L, LIMB_LEG_R, WEAPON_AP, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_EGG, WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_ROCKET, WEAPON_SHELL, WEAPON_WRECK } from './net/codec'
import { HEN_EGGS } from './sim/consts'

const container = ref<HTMLElement | null>(null)
const stats = shallowRef<HudStats | null>(null)
const noseMark = ref<HTMLElement | null>(null)
const aimMark = ref<HTMLElement | null>(null)
const error = ref<string | null>(null)
const input = shallowRef<InputController | null>(null)
const look = ref<Look>({ soft: true, contact: true, far: true, ink: true, bloom: true, reflect: true, grass: true, scorch: true })
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
  // The markers follow the frame rate, so they are placed directly instead of through the throttled stats.
  const place = (element: HTMLElement | null, at: [number, number] | null) => {
    if (!element) return
    element.style.display = at ? '' : 'none'
    if (at) element.style.transform = `translate(${at[0] * window.innerWidth}px, ${at[1] * window.innerHeight}px)`
  }
  game.onSights = (nose, aim) => {
    place(noseMark.value, nose)
    place(aimMark.value, aim)
  }
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
  window.removeEventListener('keydown', onKey)
  game?.dispose()
})

const swatch = (id: number) => cssColor(colorFor(id))
const who = (id: number) => (id === 0 ? 'the fall' : nameFor(id))
/** A side's heading on the scoreboard. */
const sideName = (team: number) => (team === 0 ? 'People' : `Team ${team}`)
const settingsUrl = `${import.meta.env.BASE_URL}settings.html`
/** What the kill feed shows between killer and victim. */
const mark = (weapon: number) =>
  weapon === WEAPON_SHELL || weapon === WEAPON_AP || weapon === WEAPON_AUTOCANNON || weapon === WEAPON_ROCKET || weapon === WEAPON_GRENADE || weapon === WEAPON_WRECK || weapon === WEAPON_BOMB || weapon === WEAPON_EGG
    ? '💥'
    : weapon === WEAPON_REVOLVER ? '›‹' : '›'

/** The HUD's stick figure's limbs, `LIMB_*` bit by bit, as lines from the shoulders or hips. */
const limbParts = [
  { bit: LIMB_ARM_L, from: [20, 18], to: [7, 32] },
  { bit: LIMB_ARM_R, from: [20, 18], to: [33, 32] },
  { bit: LIMB_LEG_L, from: [20, 36], to: [10, 60] },
  { bit: LIMB_LEG_R, from: [20, 36], to: [30, 60] },
]

const markCanvas = ref<HTMLCanvasElement | null>(null)
const backOut = (t: number) => 1 + 2.70158 * (t - 1) ** 3 + 1.70158 * (t - 1) ** 2

/** Draws the hit marker: four tapered ticks that punch out and settle, and for a kill a red X and a ring. */
function drawMarker(m: { progress: number; kill: boolean } | null | undefined) {
  const g = markCanvas.value?.getContext('2d')
  if (!g) return
  g.clearRect(0, 0, 200, 200)
  if (!m) return
  const p = m.progress
  const fade = 1 - p * p * p
  const punch = 0.55 + 0.45 * backOut(Math.min(1, p * 3.5))
  const colour = m.kill ? '#ff3030' : '#ffffff'
  g.save()
  g.translate(100, 100)
  g.globalAlpha = fade
  g.lineJoin = 'round'
  const reach = (m.kill ? 15 : 10) + 8 * p
  const length = (m.kill ? 15 : 11) * punch
  for (let k = 0; k < 4; k++) {
    g.save()
    g.rotate(Math.PI / 4 + (k * Math.PI) / 2)
    g.beginPath()
    g.moveTo(reach, -1)
    g.lineTo(reach + length, -(m.kill ? 4.5 : 3))
    g.lineTo(reach + length, m.kill ? 4.5 : 3)
    g.lineTo(reach, 1)
    g.closePath()
    g.lineWidth = 3
    g.strokeStyle = 'rgba(0,0,0,0.75)'
    g.stroke()
    g.fillStyle = colour
    g.fill()
    g.restore()
  }
  if (m.kill) {
    const r = 9 + 22 * backOut(Math.min(1, p * 3)) * 0.9
    g.lineCap = 'round'
    for (const [stroke, width] of [['rgba(0,0,0,0.7)', 8], ['#ff3030', 4.5]] as const) {
      g.strokeStyle = stroke
      g.lineWidth = width
      g.beginPath()
      g.moveTo(-r, -r)
      g.lineTo(r, r)
      g.moveTo(r, -r)
      g.lineTo(-r, r)
      g.stroke()
    }
    g.globalAlpha = Math.max(0, 1 - p * 1.6)
    g.strokeStyle = '#ff6040'
    g.lineWidth = 3 * (1 - p) + 1
    g.beginPath()
    g.arc(0, 0, 18 + 70 * p, 0, Math.PI * 2)
    g.stroke()
  }
  g.restore()
}
watch(() => stats.value?.hitMarker, (marker) => drawMarker(marker))
</script>

<template>
  <div ref="container" class="viewport" />
  <div v-if="stats" class="hud">
    <span :class="['status', stats.status.toLowerCase()]">{{ stats.status === 'CONNECTED' ? 'ONLINE' : stats.status === 'CONNECTING' ? 'CONNECTING' : 'SOLO' }}</span>
    <span v-if="stats.rtt !== null">{{ Math.round(stats.rtt) }} ms</span>
    <span>{{ stats.players }} {{ stats.players === 1 ? 'player' : 'players' }} in view</span>
    <span v-if="stats.status === 'CONNECTED' && stats.voice.mic !== 'unavailable'" :class="['voice', { talking: stats.voice.talking }]">
      {{ stats.voice.mic === 'denied' ? 'mic blocked' : stats.voice.talking ? 'talking' : 'V to talk' }}<template v-if="stats.voice.peers > 0"> · {{ stats.voice.peers }} in earshot</template>
    </span>
    <span v-if="stats.status === 'CONNECTED'">{{ stats.kills }} kills · {{ stats.deaths }} deaths</span>
    <span class="debug">tiles {{ stats.tiles }} · interp {{ Math.round(stats.interpDelayMs) }} ms · fix {{ stats.correction.toFixed(2) }} m · unacked {{ stats.pending }}</span>
  </div>
  <div v-if="stats?.prompt" class="prompt">{{ stats.touch ? stats.prompt.replace(/^E:/, 'USE:') : stats.prompt }}</div>
  <TouchControls v-if="stats?.touch && stats.locked && input" :input="input" :seat="stats.seat" :can-switch="stats.canSwitchSeat" />
  <template v-if="!stats?.touch">
  <div v-if="stats?.seat === 'driver'" class="drive-hint">W/S gas and brake · A/D steer or turn · Space handbrake · H hitch a mortar or a Pak<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'roadster'" class="drive-hint">W/S gas and brake · A/D steer · Shift boost · Space hop, hold to hover · C handbrake · H hitch a mortar or a Pak<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'hen'" class="drive-hint">W/S walk · A/D turn · Space leap, hold to flap, then glide · W in the air to fly forward · Click or F lay an egg<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'walker'" class="drive-hint">W/S walk · A/D turn · Mouse aim the guns (the body follows within 30°) · Click or F fire<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'strv'" class="drive-hint">W/S gas and brake · A/D turn the tank to aim · Mouse look · Hold right mouse for the gun sight · Click or F fire · Q {{ stats.ammo === 'HE' ? 'load AP' : 'load HE' }} · {{ stats.ammo }} {{ stats.reload < 1 ? 'reloading…' : 'loaded' }}</div>
  <div v-if="stats?.seat === 'mortar'" class="drive-hint">Mouse aim sets the bearing · W/S range {{ Math.round(stats.range) }} m · Click or F fire · {{ stats.reload < 1 ? 'reloading…' : 'loaded' }}</div>
  <div v-if="stats?.seat === 'drone'" class="drive-hint">Mouse look and steer · W fly where you look · S back off · A/D strafe · Space climb · Shift sink · Click or F detonate · E recall</div>
  <div v-if="stats?.seat === 'loiter'" class="drive-hint">Mouse steer (it banks and turns to where you look) · W/S pitch · A/D roll · Right-click swap camera · Click or F detonate · E recall</div>
  <div v-if="stats?.seat === 'pilot'" class="drive-hint">W/S lean forward and back · A/D turn · Space climb · Shift sink<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'aviator'" class="drive-hint">Mouse steer (the plane turns to where you look) · Space throttle up · Shift throttle down · W/S/A/D by hand · Click or F fire · C bomb<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  <div v-if="stats?.seat === 'passenger'" class="drive-hint">Mouse look · Click or F fire your gun · Right mouse aim · 1-5 or wheel switch · R reload<template v-if="stats.canSwitchSeat">, or switch seat when it is full</template></div>
  <div v-if="stats?.seat === 'gunner'" class="drive-hint">Mouse aim · Hold right mouse for the gun sight · Click or F fire<template v-if="stats.ammo"> · Q {{ stats.ammo === 'HE' ? 'load AP' : 'load HE' }}</template> · <template v-if="stats.ammo">{{ stats.ammo }} </template>{{ stats.jammed ? 'jammed!' : stats.heat !== null ? 'firing' : stats.reload < 1 ? 'reloading…' : 'loaded' }}<template v-if="stats.canSwitchSeat"> · R switch seat</template></div>
  </template>
  <div v-if="(stats?.seat === 'gunner' || stats?.seat === 'walker' || stats?.seat === 'strv' || stats?.seat === 'mortar') && !stats.sight" class="reticle" :class="{ loading: stats.reload < 1 || stats.jammed }" />
  <template v-if="stats?.seat === 'aviator'">
    <div ref="noseMark" class="sight"><i /><i /></div>
    <div ref="aimMark" class="aim-circle" />
    <div v-if="stats.stall" class="stall">STALL</div>
  </template>
  <template v-if="stats?.drone">
    <div class="fpv" />
    <div class="fpv-noise" :style="{ opacity: 0.06 + 0.6 * (1 - stats.drone.signal) ** 2 }" />
    <div class="fpv-cross"><i /><i /></div>
    <div class="fpv-live">● LIVE<template v-if="stats.drone.camera"> · {{ stats.drone.camera === 'down' ? 'CAM DOWN' : 'CAM FWD' }}</template></div>
    <!-- Looking straight down, the top of the screen is where it heads. -->
    <div v-if="stats.drone.camera === 'down'" class="fpv-heading">▲</div>
    <div class="gauges">
      <div class="heat air" :class="{ jammed: stats.drone.battery < 0.2 }"><div :style="{ width: stats.drone.battery * 100 + '%' }" /><span>{{ stats.drone.battery < 0.2 ? 'BATTERY LOW' : 'BATTERY' }}</span></div>
      <div class="heat air" :class="{ jammed: stats.drone.signal < 0.2 }"><div :style="{ width: stats.drone.signal * 100 + '%' }" /><span>{{ stats.drone.signal < 0.2 ? 'SIGNAL WEAK' : 'SIGNAL' }}</span></div>
    </div>
  </template>
  <div v-if="stats && stats.feedLost > 0.01" class="fpv-lost" :style="{ opacity: stats.feedLost }"><span>SIGNAL LOST</span></div>
  <div v-if="stats?.stamina && (stats.stamina.left < 1 || stats.stamina.winded)" class="gauges">
    <div class="heat stamina" :class="{ jammed: stats.stamina.winded }"><div :style="{ width: stats.stamina.left * 100 + '%' }" /><span>{{ stats.stamina.winded ? 'WINDED' : 'STAMINA' }}</span></div>
  </div>
  <div v-if="stats && stats.air !== null" class="gauges">
    <div class="heat air" :class="{ jammed: stats.air < 0.15 }"><div :style="{ width: stats.air * 100 + '%' }" /><span>{{ stats.seat === 'hen' ? (stats.air < 0.15 ? 'GLIDE' : 'WINGS') : stats.air < 0.15 ? 'AIR LOW' : 'AIR' }}</span></div>
    <div v-if="stats.eggs !== null" class="eggs"><i v-for="k in HEN_EGGS" :key="k" :class="{ gone: k > (stats.eggs ?? 0) }" /></div>
  </div>
  <div v-if="stats && stats.heat !== null" class="gauges">
    <div class="heat" :class="{ jammed: stats.jammed }"><div :style="{ width: stats.heat * 100 + '%' }" /><span>{{ stats.jammed ? 'JAMMED' : 'GUNS' }}</span></div>
    <div v-if="stats.bombs !== null" class="bombs"><i v-for="k in 4" :key="k" :class="{ gone: k > (stats.bombs ?? 0) }" /></div>
  </div>
  <div v-else-if="stats?.scoped" class="scope"><i class="scope-h" /><i class="scope-v" /></div>
  <div v-else-if="stats?.armed" class="cross" :class="{ aim: stats.aiming }" :style="stats.hitMarker ? { '--gap': (stats.aiming ? 3 : 7) + 6 * (1 - stats.hitMarker.progress) ** 2 + 'px' } : undefined"><i /><i /><i /><i /></div>
  <div v-if="stats?.hands && stats.hp <= stats.maxHp / 4" class="vignette low-hp" />
  <div v-if="stats && stats.hurt > 0.01" class="vignette" :style="{ opacity: stats.hurt }" />
  <div v-if="stats?.hurtArcs.length" class="arcs">
    <i v-for="(arc, i) in stats.hurtArcs" :key="i" :style="{ transform: `rotate(${arc.angle}rad)`, opacity: arc.alpha }" />
  </div>
  <canvas ref="markCanvas" class="hitcanvas" width="200" height="200" />
  <div v-if="stats?.callouts.length" class="callouts">
    <div v-for="line in stats.callouts" :key="line.text" :class="line.tone" :style="{ opacity: line.alpha }">{{ line.text }}<template v-if="line.count > 1"> ×{{ line.count }}</template></div>
  </div>
  <svg v-if="stats?.hands && stats.limbs" class="body-map" viewBox="0 0 40 64" aria-label="limbs">
    <circle cx="20" cy="8" r="6" />
    <line x1="20" y1="14" x2="20" y2="36" />
    <line v-for="part in limbParts" :key="part.bit" :x1="part.from[0]" :y1="part.from[1]" :x2="part.to[0]" :y2="part.to[1]"
      :class="{ lost: stats.limbs.lost & part.bit, bleeding: stats.limbs.bleeding & part.bit }" />
  </svg>
  <div v-if="stats?.hands" class="health" :class="{ low: stats.hp <= stats.maxHp / 4, hurt: stats.hurt > 0.3 }">
    <div class="health-fill" :style="{ width: (100 * stats.hp) / stats.maxHp + '%' }" />
    <span>{{ stats.hp }}</span>
  </div>
  <Hotbar
    v-if="stats?.hands"
    :slots="stats.hotbar"
    :selected="stats.selected"
    :progress="stats.progress"
    :touch="stats.touch"
    @select="(slot) => input?.selectSlot(slot)"
  />
  <Minimap v-if="stats && stats.halfExtent > 0 && !stats.touch" :x="stats.x" :z="stats.z" :yaw="stats.yaw" :half-extent="stats.halfExtent" :open="mapOpen" :blips="stats.blips" />
  <div v-if="stats?.dead" class="death">
    <h2>ELIMINATED</h2>
    <p>Respawning in {{ Math.ceil(stats.respawnIn) }}</p>
  </div>
  <div v-if="stats && (stats.notices.objective || stats.notices.messages.length)" class="scenario">
    <p v-if="stats.notices.objective" class="objective">{{ stats.notices.objective }}</p>
    <p v-for="(text, i) in stats.notices.messages" :key="i" class="message">{{ text }}</p>
  </div>
  <div v-if="stats?.notices.outcome" class="outcome" :class="stats.notices.outcome.won ? 'won' : 'lost'">
    <h2>{{ stats.notices.outcome.won ? 'VICTORY' : 'DEFEAT' }}</h2>
    <p>{{ stats.notices.outcome.text }}</p>
  </div>
  <ul v-if="stats?.feed.length" class="feed">
    <li v-for="(entry, i) in stats.feed" :key="i" :class="entry.you ?? ''">
      <template v-if="entry.killer !== 0">
        <i :style="{ background: swatch(entry.killer) }" />{{ who(entry.killer) }}<small v-if="isBot(entry.killer)" class="tag">BOT</small>
        <b>{{ mark(entry.weapon) }}</b>
      </template>
      <i :style="{ background: swatch(entry.victim) }" />{{ who(entry.victim) }}<small v-if="isBot(entry.victim)" class="tag">BOT</small><template v-if="entry.killer === 0"> took {{ who(0) }}</template>
    </li>
  </ul>
  <table v-if="stats?.board.length" class="board">
    <thead><tr><th>Player</th><th>Kills</th><th>Deaths</th></tr></thead>
    <tbody v-for="side in stats.board" :key="side.team">
      <tr v-if="stats.board.length > 1" class="side">
        <th>{{ sideName(side.team) }}</th>
        <th>{{ side.kills }}</th>
        <th>{{ side.deaths }}</th>
      </tr>
      <tr v-for="row in side.rows" :key="row.id" :class="{ you: row.you }">
        <td><i :style="{ background: swatch(row.id) }" />{{ row.you ? `${nameFor(row.id)} (you)` : nameFor(row.id) }}<small v-if="isBot(row.id)" class="tag">BOT</small></td>
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
    <p><a class="settings" :href="settingsUrl" @click.stop>Name, colour and spawn points</a></p>
    <p v-if="stats.touch" class="keys">Left thumb moves · Drag the right side to look · Buttons jump, crouch, fire, reload and use · Tap the slots to switch items</p>
    <p v-else class="keys">WASD move · Shift run · Space jump · E take, drive, ride or man a gun · 1-5 or wheel pick an item · Mouse look · Click fire · Right mouse aim (scope with the sniper or Carl Gustaf) · R reload · V talk · M map · Tab scores · Esc release</p>
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
.voice.talking { color: #138a4b; font-weight: 600; }
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
/* What armour made of a round: under the crosshair, the newest on top. */
.callouts { position: fixed; top: calc(50% + 34px); left: 50%; transform: translateX(-50%); pointer-events: none; display: flex; flex-direction: column; align-items: center; gap: 2px; }
.callouts div { font-size: 13px; font-weight: 800; letter-spacing: 0.02em; text-transform: uppercase; color: #f4f1ea; text-shadow: 0 1px 2px rgba(0, 0, 0, 0.8), 0 0 1px #000; white-space: nowrap; }
.callouts .good { color: #ffd27a; }
.callouts .bad { color: #ff7a6a; }
.callouts div:first-child { font-size: 15px; }
.reticle.loading { border-color: rgba(255, 190, 120, 0.9); border-style: dashed; }
/* A plane's pilot: where the guns point. */
.sight { position: fixed; left: 0; top: 0; width: 0; height: 0; pointer-events: none; will-change: transform; }
.aim-circle { position: fixed; left: -17px; top: -17px; width: 34px; height: 34px; box-sizing: border-box; border: 2px solid rgba(255, 255, 255, 0.9); border-radius: 50%; box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.4); pointer-events: none; will-change: transform; }
.sight i { position: absolute; background: rgba(255, 236, 160, 0.95); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.4); }
.sight i:nth-child(1) { width: 22px; height: 2px; left: -11px; top: -1px; }
.sight i:nth-child(2) { width: 2px; height: 22px; left: -1px; top: -11px; }
.stall {
  position: fixed; top: 30%; left: 50%; transform: translateX(-50%); pointer-events: none;
  font-size: 28px; font-weight: 900; letter-spacing: 0.2em; color: #ff5a5f; text-shadow: 0 2px 0 rgba(0, 0, 0, 0.4);
  animation: blink 0.5s steps(2) infinite;
}
@keyframes blink { 50% { opacity: 0.2; } }
/* Flying a drone: its camera's feed, with scanlines, a dark lens edge and static that grows as the link weakens. */
.fpv {
  position: fixed; inset: 0; pointer-events: none;
  background:
    repeating-linear-gradient(to bottom, rgba(0, 0, 0, 0.12) 0, rgba(0, 0, 0, 0.12) 1px, transparent 1px, transparent 3px),
    radial-gradient(ellipse at center, transparent 55%, rgba(0, 0, 0, 0.55) 100%);
}
.fpv-noise, .fpv-lost {
  position: fixed; inset: -50%; pointer-events: none;
  background:
    repeating-radial-gradient(circle at 17% 33%, rgba(255, 255, 255, 0.5) 0, rgba(0, 0, 0, 0.5) 1px, rgba(255, 255, 255, 0.2) 2px, transparent 3px),
    repeating-radial-gradient(circle at 71% 64%, rgba(0, 0, 0, 0.5) 0, rgba(255, 255, 255, 0.4) 1px, transparent 2px, transparent 4px);
  animation: static 0.25s steps(4) infinite;
}
.fpv-lost { background-color: #333; display: grid; place-content: center; }
.fpv-lost span { font-size: 28px; font-weight: 900; letter-spacing: 0.2em; color: #fff; text-shadow: 0 2px 0 rgba(0, 0, 0, 0.6); }
@keyframes static { 0% { transform: translate(0, 0); } 25% { transform: translate(-7%, 4%); } 50% { transform: translate(5%, -6%); } 75% { transform: translate(-3%, -2%); } }
.fpv-cross { position: fixed; top: 50%; left: 50%; width: 0; height: 0; pointer-events: none; }
.fpv-cross i { position: absolute; background: rgba(255, 255, 255, 0.85); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.4); }
.fpv-cross i:nth-child(1) { width: 18px; height: 2px; left: -9px; top: -1px; }
.fpv-cross i:nth-child(2) { width: 2px; height: 18px; left: -1px; top: -9px; }
.fpv-heading { position: fixed; top: 14px; left: 50%; transform: translateX(-50%); pointer-events: none; font-size: 14px; color: rgba(255, 255, 255, 0.85); text-shadow: 0 1px 0 rgba(0, 0, 0, 0.6); }
.fpv-live { position: fixed; top: 14px; right: 16px; pointer-events: none; font-size: 13px; font-weight: 800; color: #ff5a5f; letter-spacing: 0.1em; animation: blink 1s steps(2) infinite; }
.gauges { position: fixed; right: 16px; bottom: 16px; display: flex; flex-direction: column; align-items: flex-end; gap: 6px; pointer-events: none; }
.heat {
  position: relative; width: 160px; height: 18px; border-radius: 999px; overflow: hidden;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.heat div { height: 100%; background: linear-gradient(90deg, #f3c94b, #ff5a5f); }
.heat.jammed div { background: #ff5a5f; }
.heat.air div { background: linear-gradient(90deg, #9fd8ff, #e8f6ff); }
.heat.air.jammed div { background: #ff5a5f; }
.heat.stamina div { background: linear-gradient(90deg, #8fd16a, #d9f5a8); }
.heat.stamina.jammed div { background: #ff5a5f; }
.heat span { position: absolute; inset: 0; display: grid; place-content: center; font-size: 11px; font-weight: 800; color: #1d2b36; }
.bombs { display: flex; gap: 5px; }
.bombs i { width: 10px; height: 22px; border-radius: 5px 5px 2px 2px; background: #2b2b26; box-shadow: 0 0 0 2px rgba(255, 255, 255, 0.7); }
.bombs i.gone { opacity: 0.2; }
.eggs { display: flex; gap: 5px; }
.eggs i { width: 14px; height: 18px; border-radius: 50% 50% 50% 50% / 60% 60% 40% 40%; background: #f6efdc; box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.45); }
.eggs i.gone { opacity: 0.2; }
.health {
  position: fixed; left: 16px; bottom: 16px; width: 220px; height: 22px; border-radius: 999px; overflow: hidden; pointer-events: none;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.body-map { position: fixed; left: 16px; bottom: 46px; width: 30px; height: 48px; pointer-events: none; }
.body-map circle, .body-map line { stroke: rgba(255, 255, 255, 0.85); stroke-width: 4; stroke-linecap: round; fill: none; }
.body-map line.lost { stroke: rgba(255, 255, 255, 0.15); }
.body-map line.lost.bleeding { stroke: #ff5a5f; animation: bleeding 0.9s ease-in-out infinite; }
@keyframes bleeding { 0%, 100% { opacity: 0.35; } 50% { opacity: 1; } }
.health.hurt { box-shadow: 0 0 0 2px #ff5a5f, 0 0 12px rgba(255, 90, 95, 0.8); }
.vignette {
  position: fixed; inset: 0; pointer-events: none;
  background: radial-gradient(ellipse at center, rgba(200, 0, 0, 0) 45%, rgba(200, 0, 0, 0.55) 80%, rgba(160, 0, 0, 0.85) 100%);
}
.vignette.low-hp { opacity: 0.35; animation: heartbeat 1.1s ease-in-out infinite; }
@keyframes heartbeat { 0%, 100% { opacity: 0.2; } 30% { opacity: 0.5; } }
.arcs { position: fixed; top: 50%; left: 50%; width: 0; height: 0; pointer-events: none; }
.arcs i {
  position: absolute; left: -80px; top: -200px; width: 160px; height: 16px; border-radius: 50% 50% 0 0 / 100% 100% 0 0;
  background: linear-gradient(to bottom, rgba(255, 40, 40, 0.95), rgba(255, 40, 40, 0));
  transform-origin: 80px 200px;
}
.hitcanvas { position: fixed; top: 50%; left: 50%; width: 200px; height: 200px; margin: -100px 0 0 -100px; pointer-events: none; }
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
.scenario {
  position: fixed; top: 52px; left: 50%; transform: translateX(-50%); max-width: calc(100% - 32px); pointer-events: none;
  display: flex; flex-direction: column; align-items: center; gap: 4px; text-align: center; font-weight: 700; color: #1d2b36;
}
.scenario p { margin: 0; padding: 6px 14px; border-radius: 10px; background: rgba(255, 255, 255, 0.85); }
.scenario .objective { font-size: 15px; font-weight: 800; box-shadow: 0 0 0 2px #2ec4b6; }
.scenario .message { font-size: 14px; }
.outcome {
  position: fixed; inset: 0; display: grid; place-content: center; text-align: center; pointer-events: none;
  color: white; text-shadow: 0 2px 0 rgba(0, 0, 0, 0.35);
}
.outcome.won { background: rgba(20, 120, 80, 0.3); }
.outcome.lost { background: rgba(150, 20, 20, 0.3); }
.outcome h2 { margin: 0; font-size: clamp(32px, 8vw, 72px); letter-spacing: 0.08em; }
.outcome p { margin: 6px 0; font-size: 20px; font-weight: 700; }
.board {
  position: fixed; top: 50%; left: 50%; transform: translate(-50%, -50%); pointer-events: none; min-width: 320px;
  background: rgba(255, 255, 255, 0.92); border-radius: 14px; padding: 10px 16px; color: #1d2b36; font-weight: 700; border-collapse: collapse;
}
.board th { text-align: right; font-size: 12px; opacity: 0.6; padding: 4px 8px; }
.board th:first-child, .board td:first-child { text-align: left; }
.board td { text-align: right; padding: 4px 8px; }
.board tr.you td { color: #138a4b; }
.board tr.side th { text-align: right; font-size: 13px; opacity: 1; padding-top: 10px; border-bottom: 1px solid rgba(29, 43, 54, 0.15); }
.board tr.side th:first-child { text-align: left; }
.tag { margin-left: 6px; padding: 0 5px; border-radius: 4px; background: #1d2b36; color: #fff; font-size: 9px; font-weight: 700; letter-spacing: 0.05em; vertical-align: 1px; }
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
