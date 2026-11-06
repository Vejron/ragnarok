<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, watch } from 'vue'
import { cssColor, loadProfile, PLAYER_COLORS, sanitizeName, saveProfile, worldKey, type SpawnPoint } from '@/profile'
import { MAX_NAME_CHARS, MAX_SPAWNS } from '@/sim/consts'
import { HOTBAR_ICONS } from '@/hotbarIcons'
import { ITEM_NONE, itemDef, LOADOUT_ITEMS, STANDARD_LOADOUT } from '@/sim/items'
import { insideWorld, mapToWorld, worldToMap } from '@/sim/minimap'
import { loadManifest, WORLD_BASE, type WorldData } from '@/world'
import { defaultLook, loadLook, LOOK_OPTIONS, saveLook, type Look } from '@/render/look'
import { loadQuality, PRESETS, QUALITIES, resolveQuality, saveQuality, type Quality } from '@/render/quality'

/** Shift-click removes the spawn point nearest the click if it is this close, CSS pixels. */
const REMOVE_REACH = 24

/** Everything saves as it changes; a chip says so for a moment. */
const saved = ref(false)
let savedTimer = 0
function flashSaved() {
  saved.value = true
  clearTimeout(savedTimer)
  savedTimer = window.setTimeout(() => (saved.value = false), 1500)
}

const profile = ref(loadProfile())
watch(profile, (value) => {
  saveProfile(value)
  flashSaved()
}, { deep: true })

const look = ref<Look>(loadLook())
function toggleLook(key: keyof Look) {
  look.value = { ...look.value, [key]: !look.value[key] }
  saveLook(look.value)
  flashSaved()
}
const quality = ref<Quality>(loadQuality())
function chooseQuality(next: Quality) {
  quality.value = next
  saveQuality(next)
  flashSaved()
}
/** What auto comes to here, by name. */
const autoLevel = QUALITIES.find((q) => q.key === resolveQuality('auto'))!.label
const QUALITY_ABOUT: Record<Exclude<Quality, 'auto'>, string> = {
  high: `Sharpest: smoothed edges at up to ${PRESETS.high.maxPixelRatio}× pixels, soft shadows, thick grass, and the furthest view.`,
  medium: 'Fewer pixels on sharp screens, thinner grass and a slightly nearer horizon.',
  low: 'For an older phone: no edge smoothing, hard shadows and a nearer horizon.',
}
function resetLook() {
  look.value = defaultLook()
  saveLook(look.value)
  chooseQuality('auto')
}
const customLook = computed(() => quality.value !== 'auto' || JSON.stringify(look.value) !== JSON.stringify(defaultLook()))

const world = shallowRef<WorldData | null>(null)
const hash = ref(0)
const worldError = ref<string | null>(null)
const mapMissing = ref(false)
const notice = ref<string | null>(null)
let noticeTimer = 0

onMounted(async () => {
  try {
    const loaded = await loadManifest()
    world.value = loaded.world
    hash.value = loaded.hash
  } catch (error) {
    worldError.value = error instanceof Error ? error.message : String(error)
  }
})

const mapUrl = computed(() => (world.value ? `${WORLD_BASE}minimap.png` : ''))
const playUrl = import.meta.env.BASE_URL
const displayName = computed(() => sanitizeName(profile.value.name) || 'Player')
const outfit = computed(() => cssColor(PLAYER_COLORS[profile.value.color ?? 0]!.hex))
const colorName = computed(() => (profile.value.color === null ? 'Automatic: picked by your player number' : PLAYER_COLORS[profile.value.color]!.name))
const nameLength = computed(() => [...profile.value.name].length)

/** This world's spawn points, in the order they are used. */
const spawns = computed<SpawnPoint[]>({
  get: () => profile.value.spawns[worldKey(hash.value)] ?? [],
  set: (points) => {
    const key = worldKey(hash.value)
    const next = { ...profile.value.spawns }
    if (points.length) next[key] = points
    else delete next[key]
    profile.value = { ...profile.value, spawns: next }
  },
})

const markers = computed(() => {
  const half = world.value?.halfExtent ?? 0
  return spawns.value.map(([x, z], i) => {
    const [u, v] = worldToMap(x / 100, z / 100, half)
    return { i, left: `${u * 100}%`, top: `${v * 100}%` }
  })
})

function say(message: string) {
  notice.value = message
  clearTimeout(noticeTimer)
  noticeTimer = window.setTimeout(() => (notice.value = null), 2500)
}

function onNameInput(event: Event) {
  const value = (event.target as HTMLInputElement).value
  // Keep what is typed (spaces included) but never more than fits.
  profile.value = { ...profile.value, name: [...value].slice(0, MAX_NAME_CHARS).join('') }
}

function onNameBlur() {
  profile.value = { ...profile.value, name: sanitizeName(profile.value.name) }
}

function pickColor(index: number | null) {
  profile.value = { ...profile.value, color: index }
}

/** What each slot starts with, standard or chosen. */
const loadout = computed(() => profile.value.loadout ?? STANDARD_LOADOUT)
const customLoadout = computed(() => profile.value.loadout !== null)

/** Puts `item` in `slot`; if another slot holds it, the two swap, so an item is only ever there once. */
function pickItem(slot: number, item: number) {
  const next = [...loadout.value]
  const other = next.indexOf(item)
  if (item !== ITEM_NONE && other >= 0) next[other] = next[slot]!
  next[slot] = item
  profile.value = { ...profile.value, loadout: next }
}

function resetLoadout() {
  profile.value = { ...profile.value, loadout: null }
}

function addSpawn(event: MouseEvent) {
  const data = world.value
  if (!data) return
  const box = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const [x, z] = mapToWorld((event.clientX - box.left) / box.width, (event.clientY - box.top) / box.height, data.halfExtent)
  if (event.shiftKey) {
    // Metres per CSS pixel on this map, so the reach is the same on screen whatever its size.
    removeNearest(x, z, (REMOVE_REACH * 2 * data.halfExtent) / box.width)
    return
  }
  if (spawns.value.length >= MAX_SPAWNS) return say(`At most ${MAX_SPAWNS} spawn points. Remove one first.`)
  if (!insideWorld(x, z, data)) return say('That is outside the world.')
  spawns.value = [...spawns.value, [Math.round(x * 100), Math.round(z * 100)]]
}

function removeNearest(x: number, z: number, reach: number) {
  let best = -1
  let bestDistance = reach
  spawns.value.forEach(([sx, sz], i) => {
    const distance = Math.hypot(sx / 100 - x, sz / 100 - z)
    if (distance < bestDistance) [best, bestDistance] = [i, distance]
  })
  if (best >= 0) removeSpawn(best)
}

function removeSpawn(index: number) {
  spawns.value = spawns.value.filter((_, i) => i !== index)
}

/** Swaps spawn point `index` with the one `by` (±1) from it. */
function move(index: number, by: number) {
  const other = index + by
  if (other < 0 || other >= spawns.value.length) return
  const next = [...spawns.value]
  ;[next[other], next[index]] = [next[index]!, next[other]!]
  spawns.value = next
}

function clearSpawns() {
  spawns.value = []
}

const metres = ([x, z]: SpawnPoint) => `${Math.round(x / 100)} m E, ${Math.round(-z / 100)} m N`
</script>

<template>
  <main>
    <header>
      <div>
        <h1>Settings</h1>
        <p class="hint">Changes save as you make them, and apply the next time you join a match.</p>
      </div>
      <span class="saved" :class="{ shown: saved }" aria-live="polite">{{ saved ? 'Saved ✓' : '' }}</span>
      <a class="play" :href="playUrl">Back to the game</a>
    </header>

    <section class="card who">
      <h2 class="wide">Player</h2>
      <div class="preview" aria-hidden="true">
        <svg viewBox="0 0 60 90" width="60" height="90">
          <circle cx="30" cy="16" r="11" :fill="outfit" stroke="#1d2b36" stroke-width="3" />
          <path d="M30 28v30M30 58l-12 26M30 58l12 26M30 36l-16 12M30 36l16 12" :stroke="outfit" stroke-width="7" stroke-linecap="round" fill="none" />
        </svg>
        <span class="tag">{{ displayName }}</span>
      </div>
      <div class="fields">
        <label for="name">Name</label>
        <input
          id="name"
          :value="profile.name"
          :maxlength="MAX_NAME_CHARS * 2"
          placeholder="Player"
          autocomplete="nickname"
          spellcheck="false"
          aria-describedby="name-hint"
          @input="onNameInput"
          @blur="onNameBlur"
        />
        <p id="name-hint" class="hint">{{ nameLength }} / {{ MAX_NAME_CHARS }} characters. Leave empty to be “Player” and your number.</p>

        <fieldset class="colour">
          <legend>Colour</legend>
          <div class="swatches" role="radiogroup" aria-label="Colour">
            <button
              v-for="(color, index) in PLAYER_COLORS"
              :key="color.name"
              type="button"
              role="radio"
              :aria-checked="profile.color === index"
              :title="color.name"
              :aria-label="color.name"
              :class="{ chosen: profile.color === index }"
              :style="{ background: cssColor(color.hex) }"
              @click="pickColor(index)"
            />
          </div>
          <p class="hint">
            {{ colorName }}
            <button v-if="profile.color !== null" type="button" class="link" @click="pickColor(null)">Use automatic</button>
          </p>
        </fieldset>
      </div>
    </section>

    <section class="card">
      <div class="heading">
        <h2>Loadout</h2>
        <button v-if="customLoadout" type="button" class="reset" @click="resetLoadout">Use standard</button>
      </div>
      <p class="hint">What you carry at the start of every life, in hotbar order. Each item fits in one slot only: picking it again moves it.</p>
      <ol class="loadout">
        <li v-for="(item, slot) in loadout" :key="slot">
          <label :for="`slot-${slot}`">Slot {{ slot + 1 }}</label>
          <svg viewBox="0 0 24 24" width="48" height="48" aria-hidden="true" v-html="HOTBAR_ICONS[item] ?? ''" />
          <select :id="`slot-${slot}`" :value="item" @change="pickItem(slot, Number(($event.target as HTMLSelectElement).value))">
            <option :value="ITEM_NONE">Empty</option>
            <option v-for="choice in LOADOUT_ITEMS" :key="choice" :value="choice">{{ itemDef(choice)!.name }}</option>
          </select>
        </li>
      </ol>
    </section>

    <section class="card spawns">
      <div class="heading">
        <h2>Spawn points</h2>
        <button v-if="spawns.length" type="button" class="reset" @click="clearSpawns">Clear all</button>
      </div>
      <ul class="hint steps">
        <li>Click the map to add a point, up to {{ MAX_SPAWNS }}. Click a marker to remove it.</li>
        <li>You start at #1; each time you die you spawn at the next one.</li>
        <li>With none, you spawn near the middle. A point in water or inside a building is skipped.</li>
      </ul>
      <p v-if="worldError" class="error">Could not load the world: {{ worldError }}</p>
      <div v-else-if="world" class="layout">
        <div class="map" :class="{ missing: mapMissing }" @click="addSpawn" @contextmenu.prevent>
          <img v-if="!mapMissing" :src="mapUrl" alt="Map of the world" draggable="false" @error="mapMissing = true" />
          <span v-else class="nomap">This world has no map image yet</span>
          <button
            v-for="marker in markers"
            :key="marker.i"
            type="button"
            class="marker"
            :style="{ left: marker.left, top: marker.top, background: outfit }"
            :title="`Remove #${marker.i + 1}`"
            @click.stop="removeSpawn(marker.i)"
          >{{ marker.i + 1 }}</button>
          <span v-if="notice" class="notice">{{ notice }}</span>
        </div>
        <ol class="list">
          <li v-for="(point, index) in spawns" :key="`${index}:${point[0]},${point[1]}`">
            <b :style="{ background: outfit }">{{ index + 1 }}</b>
            <span>{{ metres(point) }}</span>
            <button type="button" :disabled="index === 0" title="Use earlier" :aria-label="`Move #${index + 1} earlier`" @click="move(index, -1)">↑</button>
            <button type="button" :disabled="index === spawns.length - 1" title="Use later" :aria-label="`Move #${index + 1} later`" @click="move(index, 1)">↓</button>
            <button type="button" title="Remove" :aria-label="`Remove #${index + 1}`" @click="removeSpawn(index)">×</button>
          </li>
          <li v-if="!spawns.length" class="empty">No spawn points: random spawns.</li>
        </ol>
      </div>
      <p v-else class="hint">Loading the map…</p>
    </section>

    <section class="card">
      <div class="heading">
        <h2>Graphics</h2>
        <button v-if="customLook" type="button" class="reset" @click="resetLook">Use defaults</button>
      </div>
      <p class="hint">These apply straight away, here and in the game's pause screen. Those marked “heavier” cost the most on a phone or an older computer.</p>
      <div class="quality" role="radiogroup" aria-label="Quality">
        <strong>Quality</strong>
        <button
          v-for="option in QUALITIES"
          :key="option.key"
          type="button"
          role="radio"
          :aria-checked="quality === option.key"
          :class="{ on: quality === option.key }"
          @click="chooseQuality(option.key)"
        >{{ option.key === 'auto' ? `Auto (${autoLevel})` : option.label }}</button>
      </div>
      <p class="hint">{{ quality === 'auto' ? `Picked for this device: ${QUALITY_ABOUT[resolveQuality('auto')]}` : QUALITY_ABOUT[quality] }}</p>
      <ul class="looks">
        <li v-for="option in LOOK_OPTIONS" :key="option.key">
          <label>
            <input type="checkbox" :checked="look[option.key]" @change="toggleLook(option.key)" />
            <span>
              <span class="name"><strong>{{ option.label }}</strong><em v-if="option.heavy" class="heavy">heavier</em></span>
              <small>{{ option.about }}</small>
            </span>
          </label>
        </li>
      </ul>
    </section>
  </main>
</template>

<style>
:root {
  --ink: #1d2b36;
  --muted: #5b6b78;
  --card: #ffffff;
  --page: #9fd3f0;
  --line: #d5e3ec;
}
html, body { margin: 0; min-height: 100%; background: var(--page); color: var(--ink); }
body { font-family: ui-rounded, 'SF Pro Rounded', system-ui, sans-serif; }
</style>

<style scoped>
main { max-width: 960px; margin: 0 auto; padding: 24px 16px 48px; display: grid; gap: 16px; }
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; flex-wrap: wrap; }
header > div { flex: 1; min-width: 220px; }
.saved {
  font-size: 14px; font-weight: 800; color: #138a4b; background: white; padding: 6px 12px; border-radius: 999px;
  opacity: 0; transition: opacity 0.2s;
}
.saved.shown { opacity: 1; }
.wide { flex-basis: 100%; }
h1 { margin: 0 0 8px; font-size: clamp(26px, 5vw, 40px); letter-spacing: 0.04em; transform: rotate(-2deg); }
h2 { margin: 0 0 4px; font-size: 20px; }
.play {
  padding: 10px 28px; border-radius: 999px; background: var(--ink); color: white; font-weight: 800; font-size: 18px;
  text-decoration: none;
}
.play:hover { background: #2f4556; }
.card { background: var(--card); border-radius: 18px; padding: 18px; box-shadow: 0 2px 10px rgba(0, 0, 0, 0.08); }
.who { display: flex; gap: 8px 24px; align-items: center; flex-wrap: wrap; }
.preview { display: grid; justify-items: center; gap: 6px; min-width: 120px; }
.tag { background: var(--ink); color: white; font-weight: 800; padding: 3px 10px; border-radius: 999px; font-size: 13px; }
.fields { flex: 1; min-width: 240px; display: grid; gap: 6px; }
label, .label { font-weight: 800; font-size: 14px; }
fieldset.colour { border: 0; margin: 10px 0 0; padding: 0; display: grid; gap: 6px; min-width: 0; }
fieldset.colour legend { font-weight: 800; font-size: 14px; padding: 0; margin-bottom: 6px; }
.link {
  font: inherit; font-weight: 800; color: #1f5fbf; background: none; border: 0; padding: 0 0 0 8px; cursor: pointer;
  text-decoration: underline;
}
input {
  font: inherit; font-size: 18px; font-weight: 700; padding: 8px 12px; border-radius: 10px; border: 2px solid var(--line);
  color: var(--ink); background: white; max-width: 320px;
}
input:focus { outline: none; border-color: #3a86ff; }
.hint { margin: 0; color: var(--muted); font-size: 13px; line-height: 1.45; }
.error { color: #b3261e; font-weight: 700; }
.heading { display: flex; align-items: center; justify-content: space-between; gap: 12px; min-height: 32px; }
.reset {
  font: inherit; font-size: 13px; font-weight: 800; padding: 5px 12px; border-radius: 999px; border: 2px solid var(--line);
  background: white; color: var(--ink); cursor: pointer;
}
.loadout { list-style: none; margin: 12px 0 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(130px, 1fr)); gap: 10px; }
.loadout li { display: grid; justify-items: center; gap: 6px; padding: 10px; border-radius: 12px; border: 2px solid var(--line); }
.loadout svg { color: var(--ink); }
.loadout select {
  font: inherit; font-size: 14px; font-weight: 700; width: 100%; padding: 6px 8px; border-radius: 8px; border: 2px solid var(--line);
  color: var(--ink); background: white;
}
.loadout select:focus { outline: none; border-color: #3a86ff; }
.swatches { display: flex; flex-wrap: wrap; gap: 8px; }
.swatches button {
  width: 36px; height: 36px; border-radius: 50%; border: 3px solid white; box-shadow: 0 0 0 2px rgba(29, 43, 54, 0.35); cursor: pointer;
  padding: 0;
}
.swatches button.chosen { box-shadow: 0 0 0 3px var(--ink); }
.swatches button:focus-visible { outline: 3px solid #3a86ff; outline-offset: 2px; }
.layout { display: grid; grid-template-columns: minmax(0, 1fr) 260px; gap: 16px; margin-top: 12px; align-items: start; }
.map {
  position: relative; aspect-ratio: 1; border-radius: 14px; overflow: hidden; background: #e9e5d9; cursor: crosshair;
  user-select: none;
}
.map img { width: 100%; height: 100%; display: block; }
.nomap { position: absolute; inset: 0; display: grid; place-content: center; color: var(--muted); font-weight: 700; }
.marker {
  position: absolute; transform: translate(-50%, -50%); width: 26px; height: 26px; border-radius: 50%;
  border: 2px solid white; box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.5), 0 2px 4px rgba(0, 0, 0, 0.3);
  color: white; font: 800 13px/22px ui-rounded, system-ui, sans-serif; text-shadow: 0 1px 1px rgba(0, 0, 0, 0.6);
  padding: 0; cursor: pointer;
}
.notice {
  position: absolute; left: 50%; bottom: 12px; transform: translateX(-50%); background: #ffe066; color: var(--ink);
  padding: 6px 12px; border-radius: 10px; font-weight: 700; font-size: 13px; pointer-events: none; max-width: calc(100% - 24px); text-align: center;
}
.list { list-style: none; margin: 0; padding: 0; display: grid; gap: 6px; }
.list li { display: flex; align-items: center; gap: 8px; font-size: 14px; }
.list li span { flex: 1; }
.list b {
  width: 24px; height: 24px; border-radius: 50%; color: white; display: grid; place-content: center; font-size: 12px;
  text-shadow: 0 1px 1px rgba(0, 0, 0, 0.6); flex: none;
}
.list button {
  width: 28px; height: 28px; border-radius: 8px; border: 2px solid var(--line); background: white; cursor: pointer;
  font-weight: 800; color: var(--ink);
}
.list button:disabled { opacity: 0.35; cursor: default; }
.list .empty { color: var(--muted); }
.steps { padding-left: 18px; margin-top: 4px; }
.quality { display: flex; flex-wrap: wrap; align-items: center; gap: 8px; margin-top: 12px; }
.quality strong { margin-right: 4px; }
.quality button { padding: 6px 14px; border-radius: 999px; border: 2px solid var(--line); background: transparent; color: inherit; font: inherit; cursor: pointer; }
.quality button.on { border-color: var(--ink); background: var(--ink); color: white; }
.looks { list-style: none; margin: 12px 0 0; padding: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 10px; }
.looks label { display: flex; gap: 10px; align-items: flex-start; padding: 10px 12px; border-radius: 12px; border: 2px solid var(--line); cursor: pointer; font-weight: 400; }
.looks input { width: 20px; height: 20px; margin: 2px 0 0; flex: none; accent-color: var(--ink); }
.looks label > span { display: grid; gap: 2px; }
.looks strong { font-size: 15px; }
.looks small { color: var(--muted); font-size: 13px; line-height: 1.4; }
.heavy {
  font-style: normal; font-size: 11px; font-weight: 800; text-transform: uppercase; letter-spacing: 0.04em;
  margin-left: 8px; padding: 1px 6px; border-radius: 6px; background: #ffe8a3; color: #6b4b00; vertical-align: 2px;
}
@media (max-width: 720px) {
  .layout { grid-template-columns: 1fr; }
  .list button { width: 40px; height: 40px; }
  .play { padding: 10px 20px; font-size: 16px; }
}
</style>
