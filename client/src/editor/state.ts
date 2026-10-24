/**
 * The editor's state and the actions that change it, shared by its panels and the map. Nothing here
 * touches Leaflet: the map watches these refs and draws them.
 */
import { computed, ref, shallowRef } from 'vue'
import { api, type Job, type WorldSummary } from './api'
import { blockKey, blockOf, rectBounds, tileAt, tileKey, type BlockStatus, type GridInfo, type TileId } from './grid'
import { emptyPlacements, ITEM_KINDS, VEHICLE_KINDS, type Placements } from './placements'
import { emptyScenario, renameName, squadsFromBots, usesName, validateScenario, type NameKind, type Scenario, type ScenarioSquad, type Spot, type Trigger } from './scenario'

export type Tool = 'pan' | 'paint' | 'erase' | 'spawn' | 'vehicle' | 'item'

/** Which side the map and the panels are working on. */
export type Mode = 'placements' | 'scenario'
/** What a click does in scenario mode. */
export type ScenarioTool = 'area' | 'bot' | 'tank' | 'route' | 'pick'

export const isPlacing = (t: Tool) => t === 'spawn' || t === 'vehicle' || t === 'item'

/** Below this zoom the cache window is too big to ask for. */
export const MIN_CACHE_ZOOM = 12

export const grid = shallowRef<GridInfo>()
export const painted = shallowRef(new Map<string, TileId>())
export const blocks = shallowRef(new Map<string, BlockStatus>())
export const tool = ref<Tool>('paint')
export const brush = ref(1)
export const placements = shallowRef<Placements>(emptyPlacements())
export const vehicleKind = ref(VEHICLE_KINDS[0]!.kind)
export const heading = ref(0)
export const itemKind = ref(ITEM_KINDS[0]!.item)
export const worlds = ref<WorldSummary[]>([])
export const name = ref('')
/** What a new world's ground is sampled at, metres: the finest the client draws well. */
const NEW_WORLD_CELL = 1
/** What the importer samples a world's ground at when its `area.json` does not say (`DEFAULT_CELL` there). */
const IMPORTER_CELL = 4
/** Metres between the world's ground height samples: one of `grid.cells`. */
export const cell = ref(NEW_WORLD_CELL)
export const dirty = ref(false)
export const job = ref<Job>()
export const message = ref('')
export const zoom = ref(0)
export const mode = ref<Mode>('placements')
export const scenario = ref<Scenario>(emptyScenario())
export const scenarioDirty = ref(false)
export const scenarioTool = ref<ScenarioTool>('area')
/** The squad the scenario tools add to. */
export const pickedSquad = ref('')
/** The spot the route tool adds waypoints to: a squad's bot or tank, by index. */
export const pickedSpot = ref<{ squad: string; kind: 'bots' | 'tanks'; index: number }>()
/** Set to ask the map to centre on a place; the map clears it. */
export const focus = ref<{ lat: number; lon: number } | undefined>()
/** Bumped when the cache may have changed (a job ended), so the map asks again. */
export const cacheTick = ref(0)

let pollTimer: number | undefined

/** The cache blocks the painted tiles need, and how many of them the cache has. */
export const needed = computed(() => {
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

/** Whether `[lat, lon]` is on a painted tile: the world's edge, for the scenario's positions. */
export function paintedAt(lat: number, lon: number): boolean {
  const g = grid.value
  if (!g) return false
  const tile = tileAt(g, lat, lon)
  return painted.value.has(tileKey(tile.gx, tile.gy))
}

/** What is wrong with the scenario, as the server would find it. */
export const scenarioIssues = computed(() => validateScenario(scenario.value, paintedAt))

export function setTiles(next: Map<string, TileId>) {
  painted.value = next
}

export function setPlacements(next: Placements) {
  placements.value = next
  dirty.value = true
}

/** Whether `[lat, lon]` is on a painted tile; if not, says so. */
export function onPainted(lat: number, lon: number): boolean {
  const g = grid.value
  if (!g) return false
  const tile = tileAt(g, lat, lon)
  if (painted.value.has(tileKey(tile.gx, tile.gy))) return true
  message.value = 'Place things on painted tiles only.'
  return false
}

function freeName(prefix: string, taken: string[]): string {
  let n = 1
  while (taken.includes(`${prefix}${n}`)) n++
  return `${prefix}${n}`
}

export function setScenario(next: Scenario) {
  scenario.value = next
  scenarioDirty.value = true
}

/** A scenario field changed. */
export function patchScenario(patch: Partial<Scenario>) {
  setScenario({ ...scenario.value, ...patch })
}

/** A squad's field changed, by its name. */
export function patchSquad(name: string, patch: Partial<ScenarioSquad>) {
  const squad = squadNamed(name)
  if (squad) setScenario({ ...scenario.value, squads: { ...scenario.value.squads, [name]: { ...squad, ...patch } } })
}

/** A trigger changed, by its position in the list. */
export function patchTrigger(index: number, patch: Partial<Trigger>) {
  setScenario({ ...scenario.value, triggers: scenario.value.triggers.map((t, i) => (i === index ? { ...t, ...patch } : t)) })
}

export function addTrigger() {
  setScenario({ ...scenario.value, triggers: [...scenario.value.triggers, { when: { start: true }, do: [] }] })
}

export function removeTrigger(index: number) {
  setScenario({ ...scenario.value, triggers: scenario.value.triggers.filter((_, i) => i !== index) })
}

export function addArea(lat: number, lon: number) {
  if (!paintedAt(lat, lon)) return (message.value = 'Place things on painted tiles only.')
  const name = freeName('area', Object.keys(scenario.value.areas))
  setScenario({ ...scenario.value, areas: { ...scenario.value.areas, [name]: { lat, lon, radius: 50 } } })
}

export function addSquad() {
  const name = freeName('squad', Object.keys(scenario.value.squads))
  setScenario({ ...scenario.value, squads: { ...scenario.value.squads, [name]: { team: 2, skill: 0.5, bots: [], tanks: [] } } })
  pickedSquad.value = name
}

/** Removes a squad, unless a trigger still uses it: says which. */
export function removeSquad(name: string): boolean {
  const users = usesName(scenario.value, 'squad', name)
  if (users.length) {
    message.value = `${name} is used by ${users.join(', ')}.`
    return false
  }
  const squads = { ...scenario.value.squads }
  delete squads[name]
  setScenario({ ...scenario.value, squads })
  if (pickedSquad.value === name) pickedSquad.value = Object.keys(squads)[0] ?? ''
  pickedSpot.value = undefined
  return true
}

export function removeArea(name: string) {
  const users = usesName(scenario.value, 'area', name)
  if (users.length) return (message.value = `${name} is used by ${users.join(', ')}.`)
  const areas = { ...scenario.value.areas }
  delete areas[name]
  setScenario({ ...scenario.value, areas })
}

/** Renames a squad, area or trigger and the triggers that name it. */
export function rename(kind: NameKind, from: string, to: string) {
  const taken = kind === 'trigger' ? scenario.value.triggers.some((t) => t.name === to) : to in (kind === 'area' ? scenario.value.areas : scenario.value.squads)
  if (taken) return (message.value = `There is already a ${kind} called "${to}".`)
  setScenario(renameName(scenario.value, kind, from, to))
  if (kind === 'squad' && pickedSquad.value === from) pickedSquad.value = to
}

export function squadNamed(name: string): ScenarioSquad | undefined {
  return scenario.value.squads[name]
}

/** Adds a bot or tank at a spot to the picked squad. */
export function addSpot(kind: 'bots' | 'tanks', lat: number, lon: number) {
  const squad = squadNamed(pickedSquad.value)
  if (!squad) return (message.value = 'Pick a squad first, or add one.')
  if (!paintedAt(lat, lon)) return (message.value = 'Place things on painted tiles only.')
  setScenario({ ...scenario.value, squads: { ...scenario.value.squads, [pickedSquad.value]: { ...squad, [kind]: [...(squad[kind] ?? []), { lat, lon }] } } })
  pickedSpot.value = { squad: pickedSquad.value, kind, index: (squad[kind] ?? []).length }
}

/** The squad spot's routes change: `change` gets its spot and gives back the new one. */
export function changeSpot(at: { squad: string; kind: 'bots' | 'tanks'; index: number }, change: (spot: Spot) => Spot) {
  const squad = squadNamed(at.squad)
  const spots = squad?.[at.kind]
  if (!squad || !spots?.[at.index]) return
  const next = spots.map((spot, i) => (i === at.index ? change(spot) : spot))
  setScenario({ ...scenario.value, squads: { ...scenario.value.squads, [at.squad]: { ...squad, [at.kind]: next } } })
}

export function removeSpot(at: { squad: string; kind: 'bots' | 'tanks'; index: number }) {
  const squad = squadNamed(at.squad)
  if (!squad) return
  const spots = (squad[at.kind] ?? []).filter((_, i) => i !== at.index)
  setScenario({ ...scenario.value, squads: { ...scenario.value.squads, [at.squad]: { ...squad, [at.kind]: spots } } })
  pickedSpot.value = undefined
}

/** Loads the world's scenario, or an empty one when it has none. */
export async function loadScenario(world: string) {
  scenario.value = (await api.scenario(world)) ?? emptyScenario()
  scenarioDirty.value = false
  pickedSquad.value = Object.keys(scenario.value.squads)[0] ?? ''
  pickedSpot.value = undefined
}

export async function saveScenario(): Promise<boolean> {
  if (scenarioIssues.value.length) return (message.value = `Fix ${scenarioIssues.value.length} problem(s) first.`), false
  if (!name.value) return (message.value = 'Save the world first, then its scenario.'), false
  try {
    await api.saveScenario(name.value, scenario.value)
    scenarioDirty.value = false
    message.value = `Saved the scenario for ${name.value}.`
    return true
  } catch (error) {
    message.value = String(error)
    return false
  }
}

/**
 * Turns the world's loose bots into squads of its scenario, and saves both: the scenario first, so
 * the bots are never in neither.
 */
export async function convertPlacedBots() {
  const before = placements.value
  const bots = before.bots
  if (!bots.length) return
  if (!name.value) return (message.value = 'Save the world first, then convert its bots.')
  const was = scenario.value
  const next = squadsFromBots(was, bots)
  const found = validateScenario(next, paintedAt)
  if (found.length) return (message.value = `Cannot convert yet: ${found[0]!.path} ${found[0]!.message}`)
  scenario.value = next
  placements.value = { ...before, bots: [] }
  if (!(await saveScenario())) {
    scenario.value = was
    placements.value = before
    return
  }
  if (await save()) message.value = `Moved ${bots.length} bots into squads. They are spawned by a trigger at the start of the scenario.`
  else message.value = 'The scenario is saved, but the world is not: save the world to finish, or the bots are placed twice.'
}

export async function loadWorlds() {
  worlds.value = await api.worlds()
}

export async function openWorld(world: string) {
  if (scenarioDirty.value && !window.confirm('Discard the unsaved scenario changes?')) return
  const loaded = await api.world(world)
  const tiles = loaded.tiles
  name.value = world
  cell.value = loaded.cell ?? IMPORTER_CELL
  placements.value = { spawns: loaded.spawns ?? [], vehicles: loaded.vehicles ?? [], items: loaded.items ?? [], bots: loaded.bots ?? [] }
  setTiles(new Map(tiles.map((t) => [tileKey(t.gx, t.gy), t])))
  dirty.value = false
  const first = tiles[0]
  if (first && grid.value) {
    const [s, w, n, e] = rectBounds(grid.value, first.gx, first.gy)
    focus.value = { lat: (s + n) / 2, lon: (w + e) / 2 }
  }
  await loadScenario(world)
  cacheTick.value++
}

function cleanName(raw: string): string {
  return raw.trim().toLowerCase().replace(/[^a-z0-9_-]+/g, '-').slice(0, 40)
}

export async function save(): Promise<boolean> {
  const world = cleanName(name.value)
  if (!world) {
    message.value = 'Give the world a name first.'
    return false
  }
  name.value = world
  try {
    await api.save(world, [...painted.value.values()], cell.value, placements.value)
    dirty.value = false
    message.value = `Saved ${world}.`
    await loadWorlds()
    return true
  } catch (error) {
    message.value = String(error)
    return false
  }
}

export async function remove(world: string) {
  if (!window.confirm(`Delete the world "${world}"?`)) return
  await api.remove(world)
  scenarioDirty.value = false
  if (name.value === world) newWorld()
  await loadWorlds()
}

export async function activate(world: string) {
  await api.activate(world)
  await loadWorlds()
  message.value = `${world} is the active world. Restart the server and reload the game.`
}

export function newWorld() {
  scenario.value = emptyScenario()
  scenarioDirty.value = false
  name.value = ''
  cell.value = NEW_WORLD_CELL
  placements.value = emptyPlacements()
  setTiles(new Map())
  dirty.value = false
}

export function clearAll() {
  if (painted.value.size && !window.confirm('Clear every painted tile?')) return
  dirty.value = true
  setTiles(new Map())
}

export async function poll() {
  try {
    job.value = await api.job()
  } catch (error) {
    message.value = String(error)
    return
  }
  if (job.value.state === 'running') pollTimer = window.setTimeout(poll, 600)
  else cacheTick.value++
}

export function stopPolling() {
  window.clearTimeout(pollTimer)
}

export async function build() {
  if (!(await save())) return
  try {
    job.value = await api.generate(name.value)
    poll()
  } catch (error) {
    message.value = String(error)
  }
}

export async function download() {
  if (!(await save())) return
  try {
    job.value = await api.fetchWorld(name.value)
    poll()
  } catch (error) {
    message.value = String(error)
  }
}
