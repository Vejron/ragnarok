<script setup lang="ts">
import { computed, ref } from 'vue'
import { circleOnSmall, viewWindow, worldToMap } from './sim/minimap'
import type { Blip } from './blips'
import { WORLD_BASE } from './world'

/** `circle`: the backpack's, metres, where it is somewhere inside (BACKPACK.md). */
const props = defineProps<{ x: number; z: number; yaw: number; halfExtent: number; open: boolean; blips: Blip[]; circle?: { x: number; z: number; radius: number } | null }>()

const url = `${WORLD_BASE}minimap.png`
/** The image is generated with the world; an older world has none, and then nothing is shown. */
const missing = ref(false)

const SMALL_PX = 180
const SMALL_METRES = 600

const small = computed(() => viewWindow(props.x, props.z, props.halfExtent, SMALL_METRES, SMALL_PX))
const smallStyle = computed(() => ({
  backgroundImage: `url(${url})`,
  backgroundSize: `${small.value.size}px ${small.value.size}px`,
  backgroundPosition: `${small.value.left}px ${small.value.top}px`,
}))
const marker = computed(() => {
  const [u, v] = worldToMap(props.x, props.z, props.halfExtent)
  return { left: `${u * 100}%`, top: `${v * 100}%` }
})
/** The marks inside the small map, placed in its pixels around the player at its centre. */
const smallBlips = computed(() => {
  const scale = SMALL_PX / SMALL_METRES
  return props.blips
    .map((blip) => ({ blip, left: SMALL_PX / 2 + (blip.x - props.x) * scale, top: SMALL_PX / 2 + (blip.z - props.z) * scale }))
    .filter(({ left, top }) => left > 4 && left < SMALL_PX - 4 && top > 4 && top < SMALL_PX - 4)
    .map(({ blip, left, top }) => ({ blip, style: blipStyle(blip, `${left}px`, `${top}px`) }))
})
const bigBlips = computed(() =>
  props.blips.map((blip) => {
    const [u, v] = worldToMap(blip.x, blip.z, props.halfExtent)
    return { blip, style: blipStyle(blip, `${u * 100}%`, `${v * 100}%`) }
  }),
)
function blipStyle(blip: Blip, left: string, top: string) {
  return { left, top, background: blip.color, opacity: blip.kind === 'ally' ? 1 : 0.25 + 0.75 * blip.strength }
}
/** The backpack's circle on the small map, or a pip on its edge pointing the way when it is out of view. */
const smallCircle = computed(() => {
  const c = props.circle
  if (!c) return null
  const { left, top, r, pip } = circleOnSmall(c.x, c.z, c.radius, props.x, props.z, SMALL_METRES, SMALL_PX)
  if (pip) return { pip: { left: `${pip.left}px`, top: `${pip.top}px` }, ring: null }
  return { pip: null, ring: { left: `${left}px`, top: `${top}px`, width: `${2 * r}px`, height: `${2 * r}px` } }
})
const bigCircle = computed(() => {
  const c = props.circle
  if (!c || props.halfExtent <= 0) return null
  const [u, v] = worldToMap(c.x, c.z, props.halfExtent)
  const across = (c.radius / props.halfExtent) * 100
  return { left: `${u * 100}%`, top: `${v * 100}%`, width: `${across}%`, height: `${across}%` }
})
// Camera forward is (-sin yaw, -cos yaw) in x/z; the arrow points up (north, -z) at yaw 0.
const arrow = computed(() => ({ transform: `translate(-50%, -50%) rotate(${(-props.yaw * 180) / Math.PI}deg)` }))
</script>

<template>
  <img v-show="false" :src="url" alt="" @error="missing = true" />
  <template v-if="!missing">
    <div v-if="!open" class="minimap" :style="smallStyle">
      <i v-if="smallCircle?.ring" class="modem" :style="smallCircle.ring" />
      <i v-if="smallCircle?.pip" class="modem-pip" :style="smallCircle.pip" />
      <i v-for="(mark, i) in smallBlips" :key="i" class="blip" :class="mark.blip.kind" :style="mark.style" />
      <i class="arrow" :style="{ transform: arrow.transform, left: '50%', top: '50%' }" />
    </div>
    <div v-else class="map">
      <img :src="url" alt="Map" />
      <i v-if="bigCircle" class="modem" :style="bigCircle" />
      <i v-for="(mark, i) in bigBlips" :key="i" class="blip" :class="mark.blip.kind" :style="mark.style" />
      <i class="arrow" :style="{ ...arrow, ...marker }" />
      <span class="hint">M close</span>
    </div>
  </template>
</template>

<style scoped>
.minimap {
  position: fixed;
  right: 12px;
  top: 12px;
  width: 180px;
  height: 180px;
  border-radius: 14px;
  border: 2px solid rgba(255, 255, 255, 0.9);
  box-shadow: 0 2px 10px rgba(0, 0, 0, 0.25);
  background-color: #e9e5d9;
  background-repeat: no-repeat;
  pointer-events: none;
  overflow: hidden;
}
.map {
  position: fixed;
  inset: 0;
  margin: auto;
  width: min(90vw, 90vh);
  height: min(90vw, 90vh);
  border-radius: 14px;
  border: 3px solid rgba(255, 255, 255, 0.95);
  box-shadow: 0 8px 40px rgba(0, 0, 0, 0.45);
  background: #e9e5d9;
  pointer-events: none;
  z-index: 2;
}
.map img { width: 100%; height: 100%; display: block; border-radius: 11px; }
.arrow {
  position: absolute;
  width: 0;
  height: 0;
  border-left: 7px solid transparent;
  border-right: 7px solid transparent;
  border-bottom: 16px solid #ff5a5f;
  filter: drop-shadow(0 0 1px #fff) drop-shadow(0 0 1px #fff);
}
.blip {
  position: absolute;
  width: 9px;
  height: 9px;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  box-shadow: 0 0 0 1.5px #fff;
}
/* The backpack's modems: a pulsing circle it is somewhere inside, or a pip on the edge pointing to it. */
.modem {
  position: absolute;
  transform: translate(-50%, -50%);
  border-radius: 50%;
  border: 2px solid rgba(255, 140, 0, 0.95);
  background: rgba(255, 140, 0, 0.18);
  box-sizing: border-box;
  animation: modem 1.6s ease-in-out infinite;
}
.modem-pip {
  position: absolute;
  width: 10px;
  height: 10px;
  border-radius: 50%;
  transform: translate(-50%, -50%);
  background: #ff8c00;
  box-shadow: 0 0 0 1.5px #fff;
  animation: modem 1.6s ease-in-out infinite;
}
@keyframes modem { 50% { opacity: 0.55; } }
/* Gunfire is a diamond, so it reads apart from an ally's dot. */
.blip.shot { border-radius: 1px; transform: translate(-50%, -50%) rotate(45deg); }
.hint {
  position: absolute;
  right: 8px;
  bottom: 8px;
  background: rgba(255, 255, 255, 0.85);
  color: #1d2b36;
  padding: 2px 8px;
  border-radius: 999px;
  font-size: 12px;
}
</style>
