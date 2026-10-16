<script setup lang="ts">
import { computed, onMounted, ref, shallowRef, watch } from 'vue'
import { cssColor, loadProfile, PLAYER_COLORS, sanitizeName, saveProfile, worldKey, type SpawnPoint } from '@/profile'
import { MAX_NAME_CHARS, MAX_SPAWNS } from '@/sim/consts'
import { insideWorld, mapToWorld, worldToMap } from '@/sim/minimap'
import { loadManifest, WORLD_BASE, type WorldData } from '@/world'

const profile = ref(loadProfile())
watch(profile, (value) => saveProfile(value), { deep: true })

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

function pickColor(index: number) {
  profile.value = { ...profile.value, color: index }
}

function addSpawn(event: MouseEvent) {
  const data = world.value
  if (!data) return
  const box = (event.currentTarget as HTMLElement).getBoundingClientRect()
  const [x, z] = mapToWorld((event.clientX - box.left) / box.width, (event.clientY - box.top) / box.height, data.halfExtent)
  if (event.shiftKey) {
    removeNearest(x, z)
    return
  }
  if (spawns.value.length >= MAX_SPAWNS) return say(`At most ${MAX_SPAWNS} spawn points. Remove one first.`)
  if (!insideWorld(x, z, data)) return say('That is outside the world.')
  spawns.value = [...spawns.value, [Math.round(x * 100), Math.round(z * 100)]]
}

function removeNearest(x: number, z: number) {
  let best = -1
  let bestDistance = Infinity
  spawns.value.forEach(([sx, sz], i) => {
    const distance = Math.hypot(sx / 100 - x, sz / 100 - z)
    if (distance < bestDistance) [best, bestDistance] = [i, distance]
  })
  if (best >= 0) removeSpawn(best)
}

function removeSpawn(index: number) {
  spawns.value = spawns.value.filter((_, i) => i !== index)
}

function moveEarlier(index: number) {
  if (index === 0) return
  const next = [...spawns.value]
  ;[next[index - 1], next[index]] = [next[index]!, next[index - 1]!]
  spawns.value = next
}

const metres = ([x, z]: SpawnPoint) => `${Math.round(x / 100)} m E, ${Math.round(-z / 100)} m N`
</script>

<template>
  <main>
    <header>
      <h1>Player settings</h1>
      <a class="play" :href="playUrl">Play</a>
    </header>

    <section class="card who">
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
          @input="onNameInput"
          @blur="onNameBlur"
        />
        <p class="hint">Up to {{ MAX_NAME_CHARS }} characters. Leave empty to be “Player” and your number.</p>

        <span class="label">Colour</span>
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
      </div>
    </section>

    <section class="card spawns">
      <h2>Spawn points</h2>
      <p class="hint">
        Click the map to add up to {{ MAX_SPAWNS }}. You start at #1, and each time you die you spawn at the next one.
        Shift-click or click a marker to remove it. With none, you spawn somewhere near the middle.
        A spot in water or inside a building is skipped.
      </p>
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
          <li v-for="(point, index) in spawns" :key="`${point[0]},${point[1]}`">
            <b :style="{ background: outfit }">{{ index + 1 }}</b>
            <span>{{ metres(point) }}</span>
            <button type="button" :disabled="index === 0" title="Use earlier" @click="moveEarlier(index)">↑</button>
            <button type="button" title="Remove" @click="removeSpawn(index)">×</button>
          </li>
          <li v-if="!spawns.length" class="empty">No spawn points: random spawns.</li>
        </ol>
      </div>
      <p v-else class="hint">Loading the map…</p>
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
header { display: flex; align-items: center; justify-content: space-between; gap: 16px; }
h1 { margin: 0; font-size: clamp(26px, 5vw, 40px); letter-spacing: 0.04em; transform: rotate(-2deg); }
h2 { margin: 0 0 4px; font-size: 20px; }
.play {
  padding: 10px 28px; border-radius: 999px; background: var(--ink); color: white; font-weight: 800; font-size: 18px;
  text-decoration: none;
}
.play:hover { background: #2f4556; }
.card { background: var(--card); border-radius: 18px; padding: 18px; box-shadow: 0 2px 10px rgba(0, 0, 0, 0.08); }
.who { display: flex; gap: 24px; align-items: center; flex-wrap: wrap; }
.preview { display: grid; justify-items: center; gap: 6px; min-width: 120px; }
.tag { background: var(--ink); color: white; font-weight: 800; padding: 3px 10px; border-radius: 999px; font-size: 13px; }
.fields { flex: 1; min-width: 240px; display: grid; gap: 6px; }
label, .label { font-weight: 800; font-size: 14px; }
.label { margin-top: 10px; }
input {
  font: inherit; font-size: 18px; font-weight: 700; padding: 8px 12px; border-radius: 10px; border: 2px solid var(--line);
  color: var(--ink); background: white; max-width: 320px;
}
input:focus { outline: none; border-color: #3a86ff; }
.hint { margin: 0; color: var(--muted); font-size: 13px; line-height: 1.45; }
.error { color: #b3261e; font-weight: 700; }
.swatches { display: flex; flex-wrap: wrap; gap: 8px; }
.swatches button {
  width: 34px; height: 34px; border-radius: 50%; border: 3px solid white; box-shadow: 0 0 0 2px var(--line); cursor: pointer;
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
  padding: 6px 12px; border-radius: 10px; font-weight: 700; font-size: 13px; pointer-events: none; white-space: nowrap;
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
@media (max-width: 720px) {
  .layout { grid-template-columns: 1fr; }
}
</style>
