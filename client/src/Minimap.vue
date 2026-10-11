<script setup lang="ts">
import { computed, ref } from 'vue'
import { viewWindow, worldToMap } from './sim/minimap'

const props = defineProps<{ x: number; z: number; yaw: number; halfExtent: number; open: boolean }>()

const url = `${import.meta.env.BASE_URL}minimap.png`
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
// Camera forward is (-sin yaw, -cos yaw) in x/z; the arrow points up (north, -z) at yaw 0.
const arrow = computed(() => ({ transform: `translate(-50%, -50%) rotate(${(-props.yaw * 180) / Math.PI}deg)` }))
</script>

<template>
  <img v-show="false" :src="url" alt="" @error="missing = true" />
  <template v-if="!missing">
    <div v-if="!open" class="minimap" :style="smallStyle">
      <i class="arrow" :style="{ transform: arrow.transform, left: '50%', top: '50%' }" />
    </div>
    <div v-else class="map">
      <img :src="url" alt="Map" />
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
