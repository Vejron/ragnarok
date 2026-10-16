<script setup lang="ts">
import L from 'leaflet'
import { computed, onBeforeUnmount, onMounted, ref, shallowRef } from 'vue'
import { api, type Job, type WorldSummary } from './api'
import { formatBytes, formatDuration, formatRate, percent, remaining, stageWidth } from './format'
import {
  emptyPlacements,
  HEADINGS,
  ITEM_KINDS,
  nearest,
  VEHICLE_KINDS,
  yawOf,
  type Placements,
} from './placements'
import { blockKey, blockOf, rectBounds, tileAt, tileKey, type BlockStatus, type GridInfo, type TileId } from './grid'

type Tool = 'pan' | 'paint' | 'erase' | 'spawn' | 'vehicle' | 'item'

/** Below this zoom the cache window is too big to ask for. */
const MIN_CACHE_ZOOM = 12
/** The 250 m grid lines are drawn from this zoom up. */
const GRID_ZOOM = 14

const mapEl = ref<HTMLDivElement>()
const grid = shallowRef<GridInfo>()
let map: L.Map | undefined
let canvas: HTMLCanvasElement | undefined

const painted = shallowRef(new Map<string, TileId>())
const blocks = shallowRef(new Map<string, BlockStatus>())
const tool = ref<Tool>('paint')
const brush = ref(1)
const placements = shallowRef<Placements>(emptyPlacements())
const vehicleKind = ref(VEHICLE_KINDS[0]!.kind)
const heading = ref(0)
const itemKind = ref(ITEM_KINDS[0]!.item)
let markers: L.LayerGroup | undefined
/** How close a click must be to a marker to pick it, in pixels. */
const PICK_REACH = 16
const worlds = ref<WorldSummary[]>([])
const name = ref('')
const dirty = ref(false)
const job = ref<Job>()
const message = ref('')
const zoom = ref(0)
let pollTimer: number | undefined
let cacheTimer: number | undefined

const ready = (gx: number, gy: number): boolean => {
  const g = grid.value
  if (!g) return false
  const [bx, by] = blockOf(g, gx, gy)
  return blocks.value.get(blockKey(bx, by))?.overpass ?? false
}

const needed = computed(() => {
  const g = grid.value
  const seen = new Map<string, boolean | undefined>()
  if (g) {
    for (const t of painted.value.values()) {
      const [bx, by] = blockOf(g, t.gx, t.gy)
      const key = blockKey(bx, by)
      seen.set(key, blocks.value.get(key)?.overpass)
    }
  }
  const all = [...seen.values()]
  return {
    blocks: all.length,
    cached: all.filter((v) => v === true).length,
    missing: all.filter((v) => v === false).length,
    unknown: all.filter((v) => v === undefined).length,
  }
})

function redraw() {
  const g = grid.value
  if (!map || !canvas || !g) return
  const size = map.getSize()
  const ratio = window.devicePixelRatio || 1
  if (canvas.width !== size.x * ratio || canvas.height !== size.y * ratio) {
    canvas.width = size.x * ratio
    canvas.height = size.y * ratio
    canvas.style.width = `${size.x}px`
    canvas.style.height = `${size.y}px`
  }
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0)
  ctx.clearRect(0, 0, size.x, size.y)
  const rect = (gx: number, gy: number, n: number): [number, number, number, number] => {
    const [s, w, nn, e] = rectBounds(g, gx, gy, n)
    const a = map!.latLngToContainerPoint([nn, w])
    const b = map!.latLngToContainerPoint([s, e])
    return [a.x, a.y, b.x - a.x, b.y - a.y]
  }
  // Cached blocks, tinted wherever they are, so the cache's reach is visible without painting.
  ctx.lineWidth = 1
  for (const b of blocks.value.values()) {
    if (!b.overpass) continue
    const [x, y, w, h] = rect(b.bx * g.blockTiles, b.by * g.blockTiles, g.blockTiles)
    ctx.fillStyle = 'rgba(46,160,67,0.14)'
    ctx.fillRect(x, y, w, h)
    ctx.strokeStyle = 'rgba(46,160,67,0.6)'
    ctx.strokeRect(x, y, w, h)
  }
  const bounds = map.getBounds()
  const nw = tileAt(g, bounds.getNorth(), bounds.getWest())
  const se = tileAt(g, bounds.getSouth(), bounds.getEast())
  if (map.getZoom() >= GRID_ZOOM && (se.gx - nw.gx + 1) * (se.gy - nw.gy + 1) < 8000) {
    ctx.strokeStyle = 'rgba(0,0,0,0.22)'
    for (let gy = nw.gy; gy <= se.gy; gy++) {
      for (let gx = nw.gx; gx <= se.gx; gx++) {
        const [x, y, w, h] = rect(gx, gy, 1)
        ctx.strokeRect(x, y, w, h)
      }
    }
  }
  for (const t of painted.value.values()) {
    if (t.gx < nw.gx - 1 || t.gx > se.gx + 1 || t.gy < nw.gy - 1 || t.gy > se.gy + 1) continue
    const [x, y, w, h] = rect(t.gx, t.gy, 1)
    ctx.fillStyle = ready(t.gx, t.gy) ? 'rgba(46,160,67,0.55)' : 'rgba(240,160,0,0.55)'
    ctx.fillRect(x, y, w, h)
    ctx.strokeStyle = 'rgba(30,30,30,0.5)'
    ctx.strokeRect(x, y, w, h)
  }
}

async function refreshCache() {
  if (!map || map.getZoom() < MIN_CACHE_ZOOM) return
  const b = map.getBounds()
  try {
    const found = await api.cache(b.getSouth(), b.getWest(), b.getNorth(), b.getEast())
    const next = new Map(blocks.value)
    for (const block of found) next.set(blockKey(block.bx, block.by), block)
    blocks.value = next
    redraw()
  } catch (error) {
    message.value = String(error)
  }
}

function scheduleCache() {
  window.clearTimeout(cacheTimer)
  cacheTimer = window.setTimeout(refreshCache, 250)
}

function setTiles(next: Map<string, TileId>) {
  painted.value = next
  redraw()
}

const isPlacing = (t: Tool) => t === 'spawn' || t === 'vehicle' || t === 'item'

function drawMarkers() {
  if (!markers) return
  markers.clearLayers()
  const icon = (text: string, cls: string, rotate = 0) =>
    L.divIcon({
      className: '',
      iconSize: [22, 22],
      iconAnchor: [11, 11],
      html: `<div class="marker ${cls}"><span style="transform:rotate(${-rotate}rad)">${text}</span>${cls === 'vehicle' ? `<i style="transform:rotate(${-rotate}rad)"></i>` : ''}</div>`,
    })
  const p = placements.value
  for (const s of p.spawns) L.marker([s.lat, s.lon], { icon: icon('●', 'spawn'), interactive: false }).addTo(markers)
  for (const v of p.vehicles) {
    const info = VEHICLE_KINDS.find((k) => k.kind === v.kind)
    L.marker([v.lat, v.lon], { icon: icon(info?.icon ?? '?', 'vehicle', v.yaw), interactive: false }).addTo(markers)
  }
  for (const i of p.items) L.marker([i.lat, i.lon], { icon: icon('◆', 'item'), interactive: false }).addTo(markers)
}

function setPlacements(next: Placements) {
  placements.value = next
  dirty.value = true
  drawMarkers()
}

function placeAt(event: PointerEvent) {
  const g = grid.value
  if (!map || !g) return
  const box = map.getContainer().getBoundingClientRect()
  const px = [event.clientX - box.left, event.clientY - box.top] as const
  const at = map.containerPointToLatLng([px[0], px[1]])
  const current = placements.value
  const kind = tool.value as 'spawn' | 'vehicle' | 'item'
  const list = kind === 'spawn' ? current.spawns : kind === 'vehicle' ? current.vehicles : current.items
  if (event.shiftKey || event.buttons === 2 || event.button === 2) {
    const points = list.map((m) => map!.latLngToContainerPoint([m.lat, m.lon]))
    const index = nearest(points, px[0], px[1], PICK_REACH)
    if (index < 0) return
    const rest = list.filter((_, i) => i !== index)
    setPlacements({ ...current, [kind === 'spawn' ? 'spawns' : kind === 'vehicle' ? 'vehicles' : 'items']: rest })
    return
  }
  const tile = tileAt(g, at.lat, at.lng)
  if (!painted.value.has(tileKey(tile.gx, tile.gy))) {
    message.value = 'Place things on painted tiles only.'
    return
  }
  const where = { lat: at.lat, lon: at.lng }
  if (kind === 'spawn') setPlacements({ ...current, spawns: [...current.spawns, where] })
  else if (kind === 'vehicle')
    setPlacements({ ...current, vehicles: [...current.vehicles, { kind: vehicleKind.value, yaw: yawOf(heading.value), ...where }] })
  else {
    const count = ITEM_KINDS.find((k) => k.item === itemKind.value)?.count ?? 1
    setPlacements({ ...current, items: [...current.items, { item: itemKind.value, count, ...where }] })
  }
}

let last: string | undefined

function paintAt(event: PointerEvent) {
  const g = grid.value
  if (!map || !g) return
  const rectEl = map.getContainer().getBoundingClientRect()
  const at = map.containerPointToLatLng([event.clientX - rectEl.left, event.clientY - rectEl.top])
  const centre = tileAt(g, at.lat, at.lng)
  if (last === tileKey(centre.gx, centre.gy)) return
  last = tileKey(centre.gx, centre.gy)
  const next = new Map(painted.value)
  const reach = Math.floor(brush.value / 2)
  const erase = tool.value === 'erase' || event.shiftKey || event.buttons === 2
  for (let dy = -reach; dy <= reach; dy++) {
    for (let dx = -reach; dx <= reach; dx++) {
      const gx = centre.gx + dx
      const gy = centre.gy + dy
      if (erase) next.delete(tileKey(gx, gy))
      else next.set(tileKey(gx, gy), { gx, gy })
    }
  }
  dirty.value = true
  setTiles(next)
}

function onDown(event: PointerEvent) {
  if (tool.value === 'pan' && !event.shiftKey) return
  if (event.button > 2) return
  event.preventDefault()
  event.stopPropagation()
  if (isPlacing(tool.value)) {
    placeAt(event)
    return
  }
  mapEl.value?.setPointerCapture(event.pointerId)
  last = undefined
  dragging = true
  paintAt(event)
}

let dragging = false

function onMove(event: PointerEvent) {
  if (dragging) paintAt(event)
}

function onUp(event: PointerEvent) {
  dragging = false
  mapEl.value?.releasePointerCapture(event.pointerId)
}

function applyTool() {
  if (!map) return
  const panning = tool.value === 'pan'
  for (const handler of [map.dragging, map.doubleClickZoom, map.boxZoom]) {
    if (panning) handler.enable()
    else handler.disable()
  }
  map.getContainer().style.cursor = panning ? '' : 'crosshair'
}

function setTool(next: Tool) {
  tool.value = next
  applyTool()
}

async function loadWorlds() {
  worlds.value = await api.worlds()
}

async function openWorld(world: string) {
  const loaded = await api.world(world)
  const tiles = loaded.tiles
  name.value = world
  placements.value = { spawns: loaded.spawns ?? [], vehicles: loaded.vehicles ?? [], items: loaded.items ?? [] }
  drawMarkers()
  setTiles(new Map(tiles.map((t) => [tileKey(t.gx, t.gy), t])))
  dirty.value = false
  const first = tiles[0]
  if (map && first && grid.value) {
    const [s, w, n, e] = rectBounds(grid.value, first.gx, first.gy)
    map.setView([(s + n) / 2, (w + e) / 2], Math.max(map.getZoom(), 15))
  }
  refreshCache()
}

function cleanName(raw: string): string {
  return raw.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').slice(0, 40)
}

async function save(): Promise<boolean> {
  const world = cleanName(name.value)
  if (!world) {
    message.value = 'Give the world a name first.'
    return false
  }
  name.value = world
  try {
    await api.save(world, [...painted.value.values()], placements.value)
    dirty.value = false
    message.value = `Saved ${world}.`
    await loadWorlds()
    return true
  } catch (error) {
    message.value = String(error)
    return false
  }
}

async function remove(world: string) {
  if (!window.confirm(`Delete the world "${world}"?`)) return
  await api.remove(world)
  if (name.value === world) {
    name.value = ''
    setTiles(new Map())
    placements.value = emptyPlacements()
    drawMarkers()
  }
  await loadWorlds()
}

async function activate(world: string) {
  await api.activate(world)
  await loadWorlds()
  message.value = `${world} is the active world. Restart the server and reload the game.`
}

function newWorld() {
  name.value = ''
  placements.value = emptyPlacements()
  drawMarkers()
  setTiles(new Map())
  dirty.value = false
}

function clearAll() {
  if (painted.value.size && !window.confirm('Clear every painted tile?')) return
  dirty.value = true
  setTiles(new Map())
}

async function poll() {
  try {
    job.value = await api.job()
  } catch (error) {
    message.value = String(error)
    return
  }
  if (job.value.state === 'running') {
    pollTimer = window.setTimeout(poll, 600)
  } else {
    refreshCache()
  }
}

async function build() {
  if (!(await save())) return
  try {
    job.value = await api.generate(name.value)
    poll()
  } catch (error) {
    message.value = String(error)
  }
}

async function download() {
  if (!(await save())) return
  try {
    job.value = await api.fetchWorld(name.value)
    poll()
  } catch (error) {
    message.value = String(error)
  }
}

onMounted(async () => {
  grid.value = await api.grid()
  const [lat, lon] = grid.value.origin
  map = L.map(mapEl.value!, { center: [lat, lon], zoom: 14, zoomControl: true })
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
  }).addTo(map)
  canvas = document.createElement('canvas')
  Object.assign(canvas.style, { position: 'absolute', left: '0', top: '0', pointerEvents: 'none', zIndex: '450' })
  map.getContainer().appendChild(canvas)
  markers = L.layerGroup().addTo(map)
  zoom.value = map.getZoom()
  map.on('move zoom resize', redraw)
  map.on('zoomend', () => (zoom.value = map!.getZoom()))
  map.on('moveend', scheduleCache)
  const container = map.getContainer()
  container.addEventListener('pointerdown', onDown, true)
  container.addEventListener('pointermove', onMove)
  container.addEventListener('pointerup', onUp)
  container.addEventListener('contextmenu', (e) => e.preventDefault())
  applyTool()
  await loadWorlds()
  const active = worlds.value.find((w) => w.active)
  if (active) await openWorld(active.name)
  refreshCache()
  poll()
})

onBeforeUnmount(() => {
  window.clearTimeout(pollTimer)
  window.clearTimeout(cacheTimer)
  map?.remove()
})
</script>

<template>
  <div class="editor">
    <aside>
      <h1>Map editor</h1>

      <section>
        <h2>World</h2>
        <input v-model="name" placeholder="world name" @input="dirty = true" />
        <div class="row">
          <button @click="save">Save{{ dirty ? ' *' : '' }}</button>
          <button @click="newWorld">New</button>
          <button @click="clearAll">Clear</button>
        </div>
        <p class="stat">{{ painted.size }} tiles, {{ ((painted.size * 250 * 250) / 1e6).toFixed(2) }} km²</p>
      </section>

      <section>
        <h2>Tool</h2>
        <div class="row">
          <button :class="{ on: tool === 'pan' }" @click="setTool('pan')">Pan</button>
          <button :class="{ on: tool === 'paint' }" @click="setTool('paint')">Paint</button>
          <button :class="{ on: tool === 'erase' }" @click="setTool('erase')">Erase</button>
        </div>
        <div class="row">
          <button :class="{ on: tool === 'spawn' }" @click="setTool('spawn')">Spawn</button>
          <button :class="{ on: tool === 'vehicle' }" @click="setTool('vehicle')">Vehicle</button>
          <button :class="{ on: tool === 'item' }" @click="setTool('item')">Item</button>
        </div>
        <label v-if="tool === 'vehicle'">
          Vehicle
          <select v-model.number="vehicleKind">
            <option v-for="k in VEHICLE_KINDS" :key="k.kind" :value="k.kind">{{ k.name }}</option>
          </select>
        </label>
        <label v-if="tool === 'vehicle'">
          Facing
          <select v-model.number="heading">
            <option v-for="(h, i) in HEADINGS" :key="h" :value="i">{{ h }}</option>
          </select>
        </label>
        <label v-if="tool === 'item'">
          Item
          <select v-model.number="itemKind">
            <option v-for="k in ITEM_KINDS" :key="k.item" :value="k.item">{{ k.name }}</option>
          </select>
        </label>
        <p v-if="isPlacing(tool)" class="hint">Click a painted tile to place; Shift or right-click removes the nearest.</p>
        <p class="stat">
          {{ placements.spawns.length }} spawns, {{ placements.vehicles.length }} vehicles, {{ placements.items.length }} items.
          Placing any of a kind replaces the random ones in the game.
        </p>
        <label>
          Brush
          <select v-model.number="brush">
            <option :value="1">1 tile</option>
            <option :value="3">3 × 3</option>
            <option :value="5">5 × 5</option>
            <option :value="9">9 × 9</option>
          </select>
        </label>
        <p class="hint">Shift or right button erases. In Pan mode, hold Shift to paint.</p>
      </section>

      <section>
        <h2>Cache</h2>
        <p class="stat">
          {{ needed.cached }} of {{ needed.blocks }} blocks (2 km) cached<span v-if="needed.unknown">, {{ needed.unknown }} not looked up (zoom in)</span>
        </p>
        <div class="row">
          <button :disabled="!painted.size || job?.state === 'running'" @click="download">
            Download {{ needed.missing }} missing
          </button>
        </div>
        <div class="row">
          <button :disabled="!painted.size || job?.state === 'running'" title="Build the world files from the cache" @click="build">
            Generate world
          </button>
        </div>
        <section v-if="job && job.state !== 'idle'" class="job" :class="job.state">
          <p class="stat">
            <strong>{{ job.kind === 'generate' ? 'Generate' : 'Download' }} {{ job.world }}</strong>: {{ job.state }}
          </p>
          <template v-if="job.kind === 'download'">
            <div class="bar"><span :style="{ width: percent(job.done, job.total) }"></span></div>
            <p class="stat">block {{ job.done }} of {{ job.total }}</p>
          </template>
          <p class="stat">
            {{ job.progress.stage || 'starting' }}<span v-if="job.progress.steps">, {{ job.progress.step }} of {{ job.progress.steps }}</span>
          </p>
          <div class="bar" :class="{ busy: job.state === 'running' && !job.progress.steps }">
            <span :style="{ width: stageWidth(job.progress.step, job.progress.steps, job.state) }"></span>
          </div>
          <p class="stat numbers">
            downloaded {{ formatBytes(job.progress.bytes) }}<span v-if="job.state === 'running'">, {{ formatRate(job.progress.bytesPerSecond) }}</span>
          </p>
          <p v-if="job.progress.cached" class="stat numbers">
            from cache {{ formatBytes(job.progress.cached) }}<span v-if="job.state === 'running'">, {{ formatRate(job.progress.cachedPerSecond) }}</span>
          </p>
          <p class="stat numbers">
            elapsed {{ formatDuration(job.elapsedMs) }}<span v-if="job.state === 'running' && job.kind === 'download' && job.done">,
            about {{ formatDuration(remaining(job.elapsedMs, job.done, job.total)) }} left</span>
          </p>
          <details v-if="job.messages.length" class="log" :open="job.state === 'failed'">
            <summary>Log ({{ job.messages.length }})</summary>
            <ul class="notes">
              <li v-for="(m, i) in job.messages" :key="i">{{ m }}</li>
            </ul>
          </details>
        </section>
        <p v-if="zoom < MIN_CACHE_ZOOM" class="hint">Zoom in to see which blocks are cached.</p>
        <p class="legend">
          <span class="sw ok"></span> cached <span class="sw miss"></span> needs download
        </p>
      </section>

      <section>
        <h2>Saved worlds</h2>
        <ul class="worlds">
          <li v-for="w in worlds" :key="w.name" :class="{ current: w.name === name }">
            <button class="link" @click="openWorld(w.name)">{{ w.name }}</button>
            <span class="count">{{ w.tiles }}</span>
            <button :class="{ on: w.active }" :disabled="!w.generated" title="Use this world in the game" @click="activate(w.name)">
              {{ w.active ? 'active' : w.generated ? 'use' : 'not built' }}
            </button>
            <button title="Delete" @click="remove(w.name)">×</button>
          </li>
          <li v-if="!worlds.length" class="hint">None yet. Paint some tiles and save.</li>
        </ul>
      </section>

      <p v-if="message" class="message">{{ message }}</p>
    </aside>
    <div ref="mapEl" class="map"></div>
  </div>
</template>

<style>
.marker {
  position: relative;
  width: 22px;
  height: 22px;
  border-radius: 50%;
  border: 2px solid #fff;
  box-shadow: 0 0 3px rgba(0, 0, 0, 0.7);
  color: #fff;
  font: bold 11px/18px system-ui, sans-serif;
  text-align: center;
  box-sizing: border-box;
}
.marker span {
  display: block;
}
.marker.spawn {
  background: #1a73e8;
}
.marker.vehicle {
  background: #c5221f;
}
.marker.item {
  background: #e8a200;
  border-radius: 4px;
}
.marker i {
  position: absolute;
  left: 4px;
  top: -9px;
  width: 0;
  height: 0;
  border: 5px solid transparent;
  border-bottom: 8px solid #c5221f;
  border-top: 0;
  transform-origin: 50% 20px;
}
html,
body {
  margin: 0;
  height: 100%;
  font: 14px system-ui, sans-serif;
}
#editor {
  height: 100%;
}
</style>

<style scoped>
.editor {
  display: flex;
  height: 100%;
}
aside {
  width: 280px;
  flex: none;
  padding: 12px;
  overflow: auto;
  box-sizing: border-box;
  background: #f6f6f4;
  border-right: 1px solid #ccc;
  color: #222;
}
.map {
  flex: 1;
  min-width: 0;
}
h1 {
  font-size: 18px;
  margin: 0 0 8px;
}
h2 {
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.06em;
  color: #666;
  margin: 14px 0 6px;
}
input,
select {
  width: 100%;
  box-sizing: border-box;
  padding: 5px 6px;
}
select {
  width: auto;
  margin-left: 6px;
}
.row {
  display: flex;
  gap: 6px;
  margin: 6px 0;
}
button {
  padding: 5px 9px;
  cursor: pointer;
}
button.on {
  background: #2ea043;
  color: #fff;
  border-color: #2ea043;
}
button.link {
  border: 0;
  background: none;
  text-align: left;
  flex: 1;
  padding: 3px 0;
  text-decoration: underline;
}
.stat,
.hint {
  margin: 4px 0;
  color: #555;
}
.hint {
  font-size: 12px;
}
.worlds {
  list-style: none;
  margin: 0;
  padding: 0;
}
.worlds li {
  display: flex;
  gap: 6px;
  align-items: center;
  padding: 2px 4px;
}
.worlds li.current {
  background: #e4ecd8;
}
.count {
  color: #777;
  font-size: 12px;
}
.notes {
  font-size: 12px;
  color: #a05a00;
  padding-left: 16px;
}
.job {
  margin: 6px 0;
}
.job.failed {
  color: #a00;
}
.job .stat {
  margin: 2px 0;
}
.numbers {
  font-variant-numeric: tabular-nums;
  font-size: 13px;
}
.bar {
  position: relative;
  height: 6px;
  margin: 4px 0;
  background: #dde3ea;
  border-radius: 3px;
  overflow: hidden;
}
.bar span {
  display: block;
  height: 100%;
  background: #4a7fd0;
  transition: width 0.4s;
}
.bar.busy span {
  width: 30%;
  position: absolute;
  animation: slide 1.2s linear infinite;
}
@keyframes slide {
  from {
    left: -30%;
  }
  to {
    left: 100%;
  }
}
.log summary {
  font-size: 12px;
  color: #777;
  cursor: pointer;
}
.message {
  color: #a00;
}
.legend {
  font-size: 12px;
}
.sw {
  display: inline-block;
  width: 12px;
  height: 12px;
  vertical-align: -2px;
  margin: 0 4px 0 8px;
}
.sw.ok {
  background: rgba(46, 160, 67, 0.7);
}
.sw.miss {
  background: rgba(240, 160, 0, 0.7);
}
</style>
