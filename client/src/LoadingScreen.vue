<script setup lang="ts">
import { computed } from 'vue'
import type { LoadProgress } from './loading'

/*
 * The screen between the page opening and the game's first frame (LOAD-TIME.md): the stages of the
 * start as a checklist, several running at once, and one bar for the whole. Its styles are in
 * `index.html`, which shows the same screen, empty, until this script has arrived to fill it.
 */
const props = defineProps<{
  /** Where the start is; null until the game has said. */
  progress: LoadProgress | null
  /** Why the game would not start, in place of what it is doing. */
  error: string | null
}>()

const stages = computed(() => props.progress?.stages ?? [])
/** The stage to name under the bar: the first still running. */
const now = computed(() => stages.value.find((stage) => stage.state === 'running')?.label ?? 'Loading the game')
const percent = computed(() => Math.round((props.progress?.fraction ?? 0) * 100))
</script>

<template>
  <div class="loading" role="status">
    <h1>RAGNAROK</h1>
    <div class="loading-bar" role="progressbar" aria-valuemin="0" aria-valuemax="100" :aria-valuenow="percent">
      <div :style="{ width: percent + '%' }" />
    </div>
    <p v-if="error" class="loading-error" role="alert">{{ error }}</p>
    <p v-else class="loading-now">{{ now }}</p>
    <ul class="loading-stages">
      <li v-for="stage in stages" :key="stage.key" :class="stage.state">
        {{ stage.label }}<template v-if="stage.of > 0 && stage.state !== 'done'"> {{ stage.done }}/{{ stage.of }}</template>
      </li>
    </ul>
  </div>
</template>
