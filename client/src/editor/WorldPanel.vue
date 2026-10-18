<script setup lang="ts">
import { activate, clearAll, dirty, name, newWorld, openWorld, painted, remove, save, worlds } from './state'
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
    <p class="text-sm text-muted">{{ painted.size }} tiles, {{ ((painted.size * 250 * 250) / 1e6).toFixed(2) }} km²</p>

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
