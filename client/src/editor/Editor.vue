<script setup lang="ts">
import { onBeforeUnmount } from 'vue'
import CachePanel from './CachePanel.vue'
import EditorMap from './EditorMap.vue'
import PlacedBotsNotice from './PlacedBotsNotice.vue'
import ScenarioPanel from './ScenarioPanel.vue'
import ToolPanel from './ToolPanel.vue'
import WorldPanel from './WorldPanel.vue'
import { message, mode, stopPolling } from './state'

onBeforeUnmount(stopPolling)
</script>

<template>
  <UApp class="h-full">
    <div class="flex h-full">
      <aside class="w-72 flex-none space-y-4 overflow-auto border-r border-default bg-muted p-3">
        <h1 class="text-lg font-semibold">Map editor</h1>
        <div class="flex gap-2">
          <UButton label="Placements" size="sm" :color="mode === 'placements' ? 'success' : 'neutral'" :variant="mode === 'placements' ? 'solid' : 'outline'" @click="mode = 'placements'" />
          <UButton label="Scenario" size="sm" :color="mode === 'scenario' ? 'success' : 'neutral'" :variant="mode === 'scenario' ? 'solid' : 'outline'" @click="mode = 'scenario'" />
        </div>
        <PlacedBotsNotice />
        <WorldPanel />
        <ToolPanel v-if="mode === 'placements'" />
        <ScenarioPanel v-else />
        <CachePanel />
        <UAlert v-if="message" :description="message" color="error" variant="subtle" />
      </aside>
      <EditorMap />
    </div>
  </UApp>
</template>
