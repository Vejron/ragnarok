<script setup lang="ts">
import { HEADINGS, ITEM_KINDS, VEHICLE_KINDS } from './placements'
import { brush, heading, isPlacing, itemKind, placements, tool, vehicleKind, type Tool } from './state'

const tools: { id: Tool; label: string }[][] = [
  [{ id: 'pan', label: 'Pan' }, { id: 'paint', label: 'Paint' }, { id: 'erase', label: 'Erase' }],
  [{ id: 'spawn', label: 'Spawn' }, { id: 'vehicle', label: 'Vehicle' }, { id: 'item', label: 'Item' }],
]
const vehicles = VEHICLE_KINDS.map((k) => ({ label: k.name, value: k.kind }))
const facings = HEADINGS.map((h, i) => ({ label: h, value: i }))
const items = ITEM_KINDS.map((k) => ({ label: k.name, value: k.item }))
const brushes = [
  { label: '1 tile', value: 1 },
  { label: '3 × 3', value: 3 },
  { label: '5 × 5', value: 5 },
  { label: '9 × 9', value: 9 },
]
</script>

<template>
  <section class="space-y-2">
    <h2 class="text-xs font-semibold uppercase tracking-wide text-muted">Tool</h2>
    <div v-for="(row, i) in tools" :key="i" class="flex gap-2">
      <UButton
        v-for="t in row"
        :key="t.id"
        :label="t.label"
        size="sm"
        :color="tool === t.id ? 'success' : 'neutral'"
        :variant="tool === t.id ? 'solid' : 'outline'"
        @click="tool = t.id"
      />
    </div>

    <USelect v-if="tool === 'vehicle'" v-model="vehicleKind" :items="vehicles" class="w-full" />
    <USelect v-if="tool === 'vehicle'" v-model="heading" :items="facings" class="w-full" />
    <USelect v-if="tool === 'item'" v-model="itemKind" :items="items" class="w-full" />

    <p v-if="isPlacing(tool)" class="text-xs text-muted">Click a painted tile to place; Shift or right-click removes the nearest.</p>

    <p class="text-sm text-muted">
      {{ placements.spawns.length }} spawns, {{ placements.vehicles.length }} vehicles, {{ placements.items.length }} items. Placing any
      spawns, vehicles or items replaces the random ones in the game. Bots are placed in Scenario mode.
    </p>
    <USelect v-model="brush" :items="brushes" class="w-full" />
    <p class="text-xs text-muted">Shift or right button erases. In Pan mode, hold Shift to paint.</p>
  </section>
</template>
