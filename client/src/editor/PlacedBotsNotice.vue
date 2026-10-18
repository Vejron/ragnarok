<script setup lang="ts">
import { convertPlacedBots, placements } from './state'
</script>

<template>
  <UAlert
    v-if="placements.bots.length"
    color="warning"
    variant="subtle"
    title="Loose bots"
    :description="`${placements.bots.length} bots are placed outside any squad. Bots now belong to squads in the scenario.`"
    :actions="[{ label: 'Convert to squads', size: 'xs', onClick: convertPlacedBots }]"
  />
</template>
