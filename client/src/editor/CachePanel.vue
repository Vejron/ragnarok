<script setup lang="ts">
import { formatBytes, formatDuration, formatRate, percent, remaining, stageWidth } from './format'
import { build, download, job, MIN_CACHE_ZOOM, needed, painted, zoom } from './state'
</script>

<template>
  <section class="space-y-2">
    <h2 class="text-xs font-semibold uppercase tracking-wide text-muted">Cache</h2>
    <p class="text-sm text-muted">
      {{ needed.cached }} of {{ needed.blocks }} blocks (2 km) cached<span v-if="needed.unknown">, {{ needed.unknown }} not looked up (zoom in)</span>
    </p>
    <div class="flex flex-col gap-2">
      <UButton :label="`Download ${needed.missing} missing`" size="sm" :disabled="!painted.size || job?.state === 'running'" @click="download" />
      <UButton
        label="Generate world"
        size="sm"
        color="neutral"
        variant="outline"
        title="Build the world files from the cache"
        :disabled="!painted.size || job?.state === 'running'"
        @click="build"
      />
    </div>

    <div v-if="job && job.state !== 'idle'" class="space-y-1 rounded border border-default p-2 text-sm" :class="{ 'text-error': job.state === 'failed' }">
      <p>
        <strong>{{ job.kind === 'generate' ? 'Generate' : 'Download' }} {{ job.world }}</strong>: {{ job.state }}
      </p>
      <template v-if="job.kind === 'download'">
        <div class="h-1.5 overflow-hidden rounded bg-accented"><span class="block h-full bg-primary transition-all" :style="{ width: percent(job.done, job.total) }"></span></div>
        <p class="text-muted">block {{ job.done }} of {{ job.total }}</p>
      </template>
      <p class="text-muted">
        {{ job.progress.stage || 'starting' }}<span v-if="job.progress.steps">, {{ job.progress.step }} of {{ job.progress.steps }}</span>
      </p>
      <UProgress v-if="job.state === 'running' && !job.progress.steps" size="xs" />
      <div v-else class="h-1.5 overflow-hidden rounded bg-accented">
        <span class="block h-full bg-primary transition-all" :style="{ width: stageWidth(job.progress.step, job.progress.steps, job.state) }"></span>
      </div>
      <p class="tabular-nums text-muted">
        downloaded {{ formatBytes(job.progress.bytes) }}<span v-if="job.state === 'running'">, {{ formatRate(job.progress.bytesPerSecond) }}</span>
      </p>
      <p v-if="job.progress.cached" class="tabular-nums text-muted">
        from cache {{ formatBytes(job.progress.cached) }}<span v-if="job.state === 'running'">, {{ formatRate(job.progress.cachedPerSecond) }}</span>
      </p>
      <p class="tabular-nums text-muted">
        elapsed {{ formatDuration(job.elapsedMs) }}<span v-if="job.state === 'running' && job.kind === 'download' && job.done">,
        about {{ formatDuration(remaining(job.elapsedMs, job.done, job.total)) }} left</span>
      </p>
      <details v-if="job.messages.length" :open="job.state === 'failed'">
        <summary class="cursor-pointer text-xs text-muted">Log ({{ job.messages.length }})</summary>
        <ul class="list-disc pl-4 text-xs text-warning">
          <li v-for="(m, i) in job.messages" :key="i">{{ m }}</li>
        </ul>
      </details>
    </div>

    <p v-if="zoom < MIN_CACHE_ZOOM" class="text-xs text-muted">Zoom in to see which blocks are cached.</p>
    <p class="flex items-center gap-1 text-xs">
      <span class="inline-block size-3 bg-green-600/70"></span> cached <span class="ml-2 inline-block size-3 bg-amber-500/70"></span> needs download
    </p>
  </section>
</template>
