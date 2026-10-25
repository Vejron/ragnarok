<script setup lang="ts">
import { computed } from 'vue'
import { activate, cell, chooseRegion, clearAll, dirty, grid, name, newWorld, openWorld, painted, region, remove, save, worlds } from './state'
import type { Region } from './grid'

const tile = computed(() => grid.value?.tile ?? 256)
/** Each resolution with what a tile's heights then weigh, so the cost of finer ground is in view. */
const cells = computed(() =>
  (grid.value?.cells ?? [1, 2, 4]).map((metres) => {
    const side = tile.value / metres + 1
    return { label: `Ground every ${metres} m (${Math.round((side * side * 2) / 1024)} KB a tile)`, value: metres }
  }),
)
const regions: { label: string; value: Region }[] = [
  { label: 'Sweden (Lantmäteriet)', value: 'sweden' },
  { label: 'France (IGN)', value: 'france' },
]
</script>

<template>
  <section class="space-y-2">
    <h2 class="text-xs font-semibold uppercase tracking-wide text-muted">World</h2>
    <UInput v-model="name" placeholder="world name" class="w-full" @input="dirty = true" />
    <div class="flex gap-2">
      <UButton :label="dirty ? 'Save *' : 'Save'" size="sm" @click="save" />
      <UButton label="New" size="sm" color="neutral" variant="outline" @click="newWorld" />
      <UButton label="Clear" size="sm" color="neutral" variant="outline" @click="clearAll" />
    </div>
    <USelect
      :model-value="region"
      :items="regions"
      class="w-full"
      title="Whose map grid and data the world is built from"
      @update:model-value="(r) => chooseRegion(r as Region)"
    />
    <USelect v-model="cell" :items="cells" class="w-full" title="How finely the ground's heights are sampled. Generate the world again to apply." @update:model-value="dirty = true" />
    <p class="text-sm text-muted">{{ painted.size }} tiles, {{ ((painted.size * tile * tile) / 1e6).toFixed(2) }} km²</p>

    <h2 class="pt-2 text-xs font-semibold uppercase tracking-wide text-muted">Saved worlds</h2>
    <ul class="space-y-1">
      <li v-for="w in worlds" :key="w.name" class="flex items-center gap-2 rounded px-1" :class="{ 'bg-elevated': w.name === name }">
        <UButton :label="w.name" variant="link" color="neutral" size="sm" class="flex-1 justify-start p-0" @click="openWorld(w.name)" />
        <span class="text-xs text-muted">{{ w.tiles }}</span>
        <UButton
          :label="w.active ? 'active' : w.generated ? 'use' : 'not built'"
          size="xs"
          :color="w.active ? 'success' : 'neutral'"
          variant="subtle"
          :disabled="!w.generated"
          title="Use this world in the game"
          @click="activate(w.name)"
        />
        <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" title="Delete" @click="remove(w.name)" />
      </li>
      <li v-if="!worlds.length" class="text-xs text-muted">None yet. Paint some tiles and save.</li>
    </ul>
  </section>
</template>
