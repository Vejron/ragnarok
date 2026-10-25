<script setup lang="ts">
import L from 'leaflet'
import { onBeforeUnmount, onMounted, ref, watch } from 'vue'
import { api } from './api'
import { blockKey, rectCorners, tileAt, tileKey, type TileId } from './grid'
import { ITEM_KINDS, nearest, routeLines, teamColour, VEHICLE_KINDS, yawOf } from './placements'
import {
  addArea, addSpot, blocks, brush, cacheTick, changeSpot, dirty, focus, grid, heading, isPlacing, itemKind, message, MIN_CACHE_ZOOM, mode,
  onPainted, painted, pickedSpot, pickedSquad, placements, poll, removeSpot, scenario, scenarioTool, setPlacements, setTiles, squadNamed, tool,
  vehicleKind, zoom, loadWorlds, openWorld, region, worlds,
} from './state'

/** The tile grid's lines are drawn from this zoom up. */
const GRID_ZOOM = 14
/** How close a click must be to a marker to pick it, in pixels. */
const PICK_REACH = 16
/** The fill of an area on the map, and of its outline. */
const AREA_COLOUR = '#7b61ff'

const mapEl = ref<HTMLDivElement>()
let map: L.Map | undefined
let canvas: HTMLCanvasElement | undefined
let markers: L.LayerGroup | undefined
let cacheTimer: number | undefined

const ready = (gx: number, gy: number): boolean => {
  const g = grid.value
  if (!g) return false
  const bx = Math.floor(gx / g.blockTiles)
  const by = Math.floor(gy / g.blockTiles)
  return blocks.value.get(blockKey(bx, by))?.overpass ?? false
}

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
  // The canvas lives in a pane that moves with the map; keep it over the container.
  L.DomUtil.setPosition(canvas, map.containerPointToLayerPoint([0, 0]))
  const ctx = canvas.getContext('2d')
  if (!ctx) return
  ctx.setTransform(ratio, 0, 0, ratio, 0, 0)
  ctx.clearRect(0, 0, size.x, size.y)
  // A square of tiles as a path of its four corners: a French grid's squares are turned a little
  // from north, so they are not rectangles on the map.
  const square = (gx: number, gy: number, n: number) => {
    ctx.beginPath()
    for (const [lat, lon] of rectCorners(g, gx, gy, n)) {
      const p = map!.latLngToContainerPoint([lat, lon])
      ctx.lineTo(p.x, p.y)
    }
    ctx.closePath()
  }
  // Cached blocks, tinted wherever they are, so the cache's reach is visible without painting.
  ctx.lineWidth = 1
  for (const b of blocks.value.values()) {
    if (!b.overpass) continue
    square(b.bx * g.blockTiles, b.by * g.blockTiles, g.blockTiles)
    ctx.fillStyle = 'rgba(46,160,67,0.14)'
    ctx.fill()
    ctx.strokeStyle = 'rgba(46,160,67,0.6)'
    ctx.stroke()
  }
  const bounds = map.getBounds()
  const corners = [
    tileAt(g, bounds.getNorth(), bounds.getWest()),
    tileAt(g, bounds.getNorth(), bounds.getEast()),
    tileAt(g, bounds.getSouth(), bounds.getWest()),
    tileAt(g, bounds.getSouth(), bounds.getEast()),
  ]
  const nw = { gx: Math.min(...corners.map((t) => t.gx)), gy: Math.min(...corners.map((t) => t.gy)) }
  const se = { gx: Math.max(...corners.map((t) => t.gx)), gy: Math.max(...corners.map((t) => t.gy)) }
  if (map.getZoom() >= GRID_ZOOM && (se.gx - nw.gx + 1) * (se.gy - nw.gy + 1) < 8000) {
    ctx.strokeStyle = 'rgba(0,0,0,0.22)'
    for (let gy = nw.gy; gy <= se.gy; gy++) {
      for (let gx = nw.gx; gx <= se.gx; gx++) {
        square(gx, gy, 1)
        ctx.stroke()
      }
    }
  }
  for (const t of painted.value.values()) {
    if (t.gx < nw.gx - 1 || t.gx > se.gx + 1 || t.gy < nw.gy - 1 || t.gy > se.gy + 1) continue
    square(t.gx, t.gy, 1)
    ctx.fillStyle = ready(t.gx, t.gy) ? 'rgba(46,160,67,0.55)' : 'rgba(240,160,0,0.55)'
    ctx.fill()
    ctx.strokeStyle = 'rgba(30,30,30,0.5)'
    ctx.stroke()
  }
}

async function refreshCache() {
  if (!map || map.getZoom() < MIN_CACHE_ZOOM) return
  const b = map.getBounds()
  try {
    const found = await api.cache(region.value, b.getSouth(), b.getWest(), b.getNorth(), b.getEast())
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
  // Loose bots from before squads: shown, not editable. The sidebar offers to turn them into squads.
  for (const bot of p.bots) {
    const colour = teamColour(bot.team)
    const { lead, loop } = routeLines(bot)
    const line = { color: colour, weight: 2, opacity: 0.6, interactive: false }
    if (lead.length) L.polyline(lead, { ...line, dashArray: '4 6' }).addTo(markers)
    if (loop.length) L.polyline(loop, line).addTo(markers)
    L.marker([bot.lat, bot.lon], {
      icon: L.divIcon({
        className: '',
        iconSize: [22, 22],
        iconAnchor: [11, 11],
        html: `<div class="marker bot" style="background:${colour}" title="loose bot, team ${bot.team}, skill ${bot.skill}">${bot.team}</div>`,
      }),
      interactive: false,
      zIndexOffset: 1000,
    }).addTo(markers)
  }
}

function placeAt(event: PointerEvent) {
  const g = grid.value
  if (!map || !g) return
  const box = map.getContainer().getBoundingClientRect()
  const px = [event.clientX - box.left, event.clientY - box.top] as const
  const at = map.containerPointToLatLng([px[0], px[1]])
  const removing = event.shiftKey || event.buttons === 2 || event.button === 2
  const current = placements.value
  const kind = tool.value as 'spawn' | 'vehicle' | 'item'
  const list = kind === 'spawn' ? current.spawns : kind === 'vehicle' ? current.vehicles : current.items
  if (removing) {
    const points = list.map((m) => map!.latLngToContainerPoint([m.lat, m.lon]))
    const index = nearest(points, px[0], px[1], PICK_REACH)
    if (index < 0) return
    const rest = list.filter((_, i) => i !== index)
    setPlacements({ ...current, [kind === 'spawn' ? 'spawns' : kind === 'vehicle' ? 'vehicles' : 'items']: rest })
    return
  }
  if (!onPainted(at.lat, at.lng)) return
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
  // Scenario clicks come from Leaflet's own click event, so the map can still be dragged.
  if (mode.value === 'scenario') return
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
  const panning = mode.value === 'scenario' || tool.value === 'pan'
  for (const handler of [map.dragging, map.doubleClickZoom, map.boxZoom]) {
    if (panning) handler.enable()
    else handler.disable()
  }
  map.getContainer().style.cursor = panning ? '' : 'crosshair'
}

/** The spots of every squad, with the one picked drawn bigger, and their routes. */
function drawScenario() {
  if (!markers) return
  markers.clearLayers()
  for (const [name, area] of Object.entries(scenario.value.areas)) {
    L.circle([area.lat, area.lon], { radius: area.radius, color: AREA_COLOUR, weight: 2, fillOpacity: 0.12, interactive: false }).addTo(markers)
    L.marker([area.lat, area.lon], {
      icon: L.divIcon({ className: '', iconSize: [0, 0], html: `<div class="area-label">${name}</div>` }),
      interactive: false,
    }).addTo(markers)
  }
  for (const [squad, def] of Object.entries(scenario.value.squads)) {
    const colour = teamColour(def.team)
    for (const kind of ['bots', 'tanks'] as const) {
      ;(def[kind] ?? []).forEach((spot, index) => {
        const picked = pickedSpot.value?.squad === squad && pickedSpot.value.kind === kind && pickedSpot.value.index === index
        const line = { color: colour, weight: picked ? 4 : 2, opacity: picked ? 0.95 : 0.6, interactive: false }
        const points: [number, number][] = (spot.route ?? []).map((p) => [p.lat, p.lon])
        if (points.length) L.polyline([[spot.lat, spot.lon], ...points], { ...line, dashArray: kind === 'tanks' ? '8 4' : undefined }).addTo(markers!)
        L.marker([spot.lat, spot.lon], {
          icon: L.divIcon({
            className: '',
            iconSize: [22, 22],
            iconAnchor: [11, 11],
            html: `<div class="marker ${kind === 'tanks' ? 'tank' : 'bot'}${picked ? ' picked' : ''}" style="background:${colour}" title="${squad} ${kind === 'tanks' ? 'tank' : 'bot'} ${index + 1}">${kind === 'tanks' ? 'T' : def.team}</div>`,
          }),
          interactive: false,
          zIndexOffset: 1000,
        }).addTo(markers!)
      })
    }
  }
}

/** Every bot and tank in the scenario, nearest to a point on the screen first. */
function nearestSpot(px: readonly [number, number]): { squad: string; kind: 'bots' | 'tanks'; index: number } | undefined {
  const all: { squad: string; kind: 'bots' | 'tanks'; index: number; at: L.Point }[] = []
  for (const [squad, def] of Object.entries(scenario.value.squads)) {
    for (const kind of ['bots', 'tanks'] as const) {
      ;(def[kind] ?? []).forEach((spot, index) => all.push({ squad, kind, index, at: map!.latLngToContainerPoint([spot.lat, spot.lon]) }))
    }
  }
  const i = nearest(
    all.map((s) => s.at),
    px[0],
    px[1],
    PICK_REACH,
  )
  return i < 0 ? undefined : all[i]
}

/** A click in scenario mode: place with the scenario tool, pick a spot, add a waypoint, or remove with Shift. */
function onScenarioClick(event: L.LeafletMouseEvent) {
  const original = event.originalEvent
  const removing = original.shiftKey
  const px = [event.containerPoint.x, event.containerPoint.y] as const
  const { lat, lng: lon } = event.latlng
  const hit = nearestSpot(px)
  if (removing) {
    if (hit) removeSpot(hit)
    return
  }
  if (scenarioTool.value === 'area') return addArea(lat, lon)
  if (scenarioTool.value === 'pick') {
    if (hit) {
      pickedSquad.value = hit.squad
      pickedSpot.value = hit
    }
    return
  }
  if (scenarioTool.value === 'bot' || scenarioTool.value === 'tank') {
    if (hit && hit.kind === (scenarioTool.value === 'tank' ? 'tanks' : 'bots')) {
      pickedSquad.value = hit.squad
      pickedSpot.value = hit
      return
    }
    return addSpot(scenarioTool.value === 'tank' ? 'tanks' : 'bots', lat, lon)
  }
  // The route tool: a waypoint on the picked spot's route; a click on the spot itself is its own point.
  const at = pickedSpot.value
  const spot = at ? squadNamed(at.squad)?.[at.kind]?.[at.index] : undefined
  if (!at || !spot) return (message.value = 'Pick a bot or tank first: click one with the Pick tool.')
  const onSpot = hit && hit.squad === at.squad && hit.kind === at.kind && hit.index === at.index
  if (!onSpot && !onPainted(lat, lon)) return
  changeSpot(at, (s) => ({ ...s, route: [...(s.route ?? []), onSpot ? { lat: s.lat, lon: s.lon } : { lat, lon }] }))
}

function onScenarioRemove(event: L.LeafletMouseEvent) {
  const hit = nearestSpot([event.containerPoint.x, event.containerPoint.y])
  if (hit) removeSpot(hit)
}

watch([painted, blocks], redraw)
watch(placements, () => mode.value === 'placements' && drawMarkers())
watch([mode, scenario, pickedSpot], () => (mode.value === 'scenario' ? drawScenario() : drawMarkers()), { deep: true })
watch(mode, applyTool)
watch(tool, applyTool)
watch(cacheTick, refreshCache)
// Another region's grid: the cached blocks and the view belong to the old one.
watch(grid, (g, before) => {
  if (!g || !before || g.region === before.region || !map) return
  blocks.value = new Map()
  map.setView(g.origin, Math.min(map.getZoom(), 7))
  redraw()
})
watch(focus, (at) => {
  if (!at || !map) return
  map.setView([at.lat, at.lon], Math.max(map.getZoom(), 15))
  focus.value = undefined
})

onMounted(async () => {
  grid.value = await api.grid(region.value)
  const [lat, lon] = grid.value.origin
  map = L.map(mapEl.value!, { center: [lat, lon], zoom: 14, zoomControl: true })
  L.tileLayer('https://tile.openstreetmap.org/{z}/{x}/{y}.png', {
    maxZoom: 19,
    attribution: '&copy; <a href="https://www.openstreetmap.org/copyright">OpenStreetMap</a> contributors',
  }).addTo(map)
  // Painted tiles go over the map tiles but under routes (overlay pane, 400) and markers.
  const tilesPane = map.createPane('painted')
  Object.assign(tilesPane.style, { zIndex: '350', pointerEvents: 'none' })
  canvas = document.createElement('canvas')
  Object.assign(canvas.style, { position: 'absolute', left: '0', top: '0', pointerEvents: 'none' })
  tilesPane.appendChild(canvas)
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
  map.on('click', (e) => mode.value === 'scenario' && onScenarioClick(e))
  map.on('contextmenu', (e) => mode.value === 'scenario' && onScenarioRemove(e))
  applyTool()
  await loadWorlds()
  const active = worlds.value.find((w) => w.active)
  if (active) await openWorld(active.name)
  refreshCache()
  poll()
})

onBeforeUnmount(() => {
  window.clearTimeout(cacheTimer)
  map?.remove()
})
</script>

<template>
  <div ref="mapEl" class="map"></div>
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
.marker.bot {
  border-radius: 4px;
}
.marker.tank {
  border-radius: 4px;
  border-style: dashed;
}
.area-label {
  transform: translate(-50%, -50%);
  white-space: nowrap;
  font: bold 12px system-ui, sans-serif;
  color: #3d2fa0;
  text-shadow: 0 0 3px #fff;
}
.marker.bot.picked {
  border: 3px solid #ffeb3b;
  line-height: 16px;
}
.waypoint {
  width: 16px;
  height: 16px;
  border-radius: 50%;
  border: 1px solid #fff;
  box-sizing: border-box;
  color: #fff;
  font: bold 9px/14px system-ui, sans-serif;
  text-align: center;
  box-shadow: 0 0 2px rgba(0, 0, 0, 0.7);
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
</style>

<style scoped>
.map {
  flex: 1;
  min-width: 0;
}
</style>
