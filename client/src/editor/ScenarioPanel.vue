<script setup lang="ts">
import { computed } from 'vue'
import { api } from './api'
import { LOADOUT_ITEMS, type Action, type NameKind, type When } from './scenario'
import {
  addSquad, addTrigger, message, name, patchScenario, patchSquad, patchTrigger, pickedSquad, rename, removeArea, removeSquad, removeTrigger,
  scenario, scenarioDirty, scenarioIssues, saveScenario, scenarioTool, setScenario, type ScenarioTool,
} from './state'

const squadNames = computed(() => Object.keys(scenario.value.squads))
const areaNames = computed(() => Object.keys(scenario.value.areas))
const squadItems = computed(() => squadNames.value.map((n) => ({ label: n, value: n })))
const areaItems = computed(() => areaNames.value.map((n) => ({ label: n, value: n })))
const namedTriggers = computed(() => scenario.value.triggers.flatMap((t) => (t.name ? [{ label: t.name, value: t.name }] : [])))
const squad = computed(() => scenario.value.squads[pickedSquad.value])
const teams = [1, 2, 3, 4].map((t) => ({ label: `Team ${t}`, value: t }))
const actionKinds = ['spawn', 'remove', 'say', 'objective', 'win', 'lose'].map((k) => ({ label: k, value: k }))
const tools: { id: ScenarioTool; label: string; hint: string }[] = [
  { id: 'area', label: 'Area', hint: 'Click the map to place an area' },
  { id: 'bot', label: 'Bot', hint: 'Click the map to add a bot to the picked squad' },
  { id: 'tank', label: 'Tank', hint: 'Click the map to add a tank to the picked squad' },
  { id: 'route', label: 'Route', hint: 'Click to add a waypoint to the picked bot or tank' },
  { id: 'pick', label: 'Pick', hint: 'Click a bot or tank to pick it' },
]
const loadout = computed({
  get: () => squad.value?.loadout ?? [],
  set: (items: string[]) => patchSquad(pickedSquad.value, { loadout: items.length ? items : undefined }),
})
const loadoutItems = LOADOUT_ITEMS

/** A checkbox's value: only a tick counts as on. */
const checked = (v: boolean | 'indeterminate') => v === true

/** Renames a name, or sets a new one when it had none. */
function renameTrigger(index: number, to: string) {
  const from = scenario.value.triggers[index]?.name
  if (!from) patchTrigger(index, { name: to.trim() || undefined })
  else rename('trigger', from, to.trim())
}

function renameKind(kind: NameKind, from: string, to: string) {
  rename(kind, from, to.trim())
}

/** Sets a condition on or off. Turning one on takes a sensible first value. */
function condition(index: number, key: 'after' | 'enter' | 'cleared' | 'done', on: boolean) {
  const when = { ...scenario.value.triggers[index]!.when }
  if (!on) {
    delete when[key]
    if (key === 'enter') delete when.by
  } else if (key === 'after') when.after = 0
  else if (key === 'enter') when.enter = areaNames.value[0] ?? ''
  else if (key === 'cleared') when.cleared = squadNames.value[0] ?? ''
  else when.done = namedTriggers.value[0]?.value ?? ''
  patchTrigger(index, { when })
}

function setWhen(index: number, key: keyof When, value: unknown) {
  patchTrigger(index, { when: { ...scenario.value.triggers[index]!.when, [key]: value } })
}

function setAction(index: number, at: number, action: Action) {
  const list = scenario.value.triggers[index]!.do.map((a, i) => (i === at ? action : a))
  patchTrigger(index, { do: list })
}

function actionKind(action: Action): string {
  return Object.keys(action)[0]!
}

function actionValue(action: Action): string {
  return Object.values(action)[0] as string
}

function makeAction(kind: string, value: string): Action {
  return { [kind]: value } as Action
}

function addAction(index: number) {
  patchTrigger(index, { do: [...scenario.value.triggers[index]!.do, makeAction('say', '')] })
}

function dropAction(index: number, at: number) {
  patchTrigger(index, { do: scenario.value.triggers[index]!.do.filter((_, i) => i !== at) })
}

async function removeScenario() {
  if (!name.value || !window.confirm('Remove the scenario for this world?')) return
  await api.removeScenario(name.value)
  setScenario({ restart: 20, areas: {}, squads: {}, triggers: [] })
  scenarioDirty.value = false
  message.value = 'Removed the scenario.'
}

function removeSquadNow() {
  if (!pickedSquad.value) return
  if (window.confirm(`Delete squad ${pickedSquad.value}?`)) removeSquad(pickedSquad.value)
}
</script>

<template>
  <section class="space-y-3">
    <h2 class="text-xs font-semibold uppercase tracking-wide text-muted">Scenario</h2>

    <div class="flex flex-wrap gap-2">
      <UButton
        v-for="t in tools"
        :key="t.id"
        :label="t.label"
        size="sm"
        :title="t.hint"
        :color="scenarioTool === t.id ? 'success' : 'neutral'"
        :variant="scenarioTool === t.id ? 'solid' : 'outline'"
        @click="scenarioTool = t.id"
      />
    </div>
    <p class="text-xs text-muted">{{ tools.find((t) => t.id === scenarioTool)?.hint }}. Shift-click or right-click removes the nearest bot or tank.</p>

    <div class="flex items-center gap-2">
      <UButton :label="scenarioDirty ? 'Save scenario *' : 'Save scenario'" size="sm" :disabled="!!scenarioIssues.length" @click="saveScenario" />
      <UButton label="Remove" size="sm" color="error" variant="outline" :disabled="!name" title="Delete the scenario file" @click="removeScenario" />
    </div>
    <div v-if="scenarioIssues.length" class="space-y-1 rounded border border-error p-2 text-xs text-error">
      <p class="font-semibold">{{ scenarioIssues.length }} problem{{ scenarioIssues.length === 1 ? '' : 's' }}, not saved yet</p>
      <p v-for="issue in scenarioIssues.slice(0, 6)" :key="issue.path + issue.message">{{ issue.path }}: {{ issue.message }}</p>
    </div>

    <label class="block text-sm">
      Restart after (seconds)
      <UInput
        :model-value="scenario.restart ?? 20"
        type="number"
        min="0"
        class="mt-1 w-full"
        @update:model-value="(v: string | number) => patchScenario({ restart: Number(v) })"
      />
    </label>

    <h3 class="pt-2 text-sm font-semibold">Areas</h3>
    <ul class="space-y-2">
      <li v-for="(area, name) in scenario.areas" :key="name" class="space-y-1 rounded bg-elevated p-2 text-sm">
        <div class="flex items-center gap-2">
          <UInput :model-value="name" class="flex-1" @change="(e: Event) => renameKind('area', name, (e.target as HTMLInputElement).value)" />
          <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" title="Remove" @click="removeArea(name)" />
        </div>
        <label class="flex items-center gap-2 text-xs text-muted">
          Radius (m)
          <UInput
            :model-value="area.radius"
            type="number"
            min="1"
            class="w-24"
            @update:model-value="(v: string | number) => patchScenario({ areas: { ...scenario.areas, [name]: { ...area, radius: Number(v) } } })"
          />
        </label>
      </li>
      <li v-if="!areaNames.length" class="text-xs text-muted">No areas. Use the Area tool on the map.</li>
    </ul>

    <h3 class="pt-2 text-sm font-semibold">Squads</h3>
    <div class="flex gap-2">
      <USelect v-model="pickedSquad" :items="squadItems" placeholder="Pick a squad" class="flex-1" />
      <UButton label="New" size="sm" color="neutral" variant="outline" @click="addSquad" />
    </div>
    <div v-if="squad" class="space-y-2 rounded bg-elevated p-2 text-sm">
      <div class="flex items-center gap-2">
        <UInput :model-value="pickedSquad" class="flex-1" @change="(e: Event) => renameKind('squad', pickedSquad, (e.target as HTMLInputElement).value)" />
        <UButton icon="i-lucide-trash-2" size="xs" color="error" variant="ghost" title="Delete squad" @click="removeSquadNow" />
      </div>
      <p class="text-xs text-muted">{{ squad.bots?.length ?? 0 }} bots, {{ squad.tanks?.length ?? 0 }} tanks</p>
      <USelect :model-value="squad.team" :items="teams" class="w-full" @update:model-value="(v: number) => patchSquad(pickedSquad, { team: v })" />
      <label class="flex items-center gap-3">
        Skill
        <USlider :model-value="squad.skill" :min="0" :max="1" :step="0.1" class="flex-1" @update:model-value="(v: number | undefined) => patchSquad(pickedSquad, { skill: v ?? 0 })" />
        <span class="w-8 text-right tabular-nums">{{ squad.skill.toFixed(1) }}</span>
      </label>
      <div>
        <p class="mb-1 text-xs text-muted">Loadout (up to 5 items, one a slot; none is the standard)</p>
        <USelectMenu v-model="loadout" :items="loadoutItems" multiple class="w-full" />
      </div>
      <UCheckbox
        :model-value="!!squad.respawn"
        label="Respawn at home after dying"
        @update:model-value="(v) => patchSquad(pickedSquad, { respawn: checked(v) || undefined })"
      />
    </div>
    <p v-else class="text-xs text-muted">No squad picked. Add one, then place its bots and tanks on the map.</p>

    <h3 class="pt-2 text-sm font-semibold">Triggers</h3>
    <ol class="space-y-3">
      <li v-for="(trigger, index) in scenario.triggers" :key="index" class="space-y-2 rounded border border-default p-2 text-sm">
        <div class="flex items-center gap-2">
          <span class="text-xs text-muted">{{ index + 1 }}</span>
          <UInput :model-value="trigger.name ?? ''" placeholder="name (optional)" class="flex-1" @change="(e: Event) => renameTrigger(index, (e.target as HTMLInputElement).value)" />
          <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" title="Remove trigger" @click="removeTrigger(index)" />
        </div>

        <div class="space-y-1">
          <p class="text-xs font-semibold text-muted">When</p>
          <label class="flex items-center gap-2"><UCheckbox :model-value="!!trigger.when.start" label="the scenario starts" @update:model-value="(v) => setWhen(index, 'start', checked(v) || undefined)" /></label>
          <div class="flex items-center gap-2">
            <UCheckbox :model-value="trigger.when.after !== undefined" label="after" @update:model-value="(v) => condition(index, 'after', checked(v))" />
            <UInput v-if="trigger.when.after !== undefined" :model-value="trigger.when.after" type="number" min="0" class="w-20" @update:model-value="(v: string | number) => setWhen(index, 'after', Number(v))" />
            <span v-if="trigger.when.after !== undefined" class="text-xs text-muted">seconds</span>
          </div>
          <div class="flex items-center gap-2">
            <UCheckbox :model-value="trigger.when.enter !== undefined" label="someone enters" @update:model-value="(v) => condition(index, 'enter', checked(v))" />
            <USelect v-if="trigger.when.enter !== undefined" :model-value="trigger.when.enter" :items="areaItems" @update:model-value="(v: string) => setWhen(index, 'enter', v)" />
          </div>
          <div v-if="trigger.when.enter !== undefined" class="ml-6 flex items-center gap-2">
            <UCheckbox :model-value="trigger.when.by !== undefined" label="only from squad" @update:model-value="(v) => setWhen(index, 'by', checked(v) ? squadNames[0] ?? '' : undefined)" />
            <USelect v-if="trigger.when.by !== undefined" :model-value="trigger.when.by" :items="squadItems" @update:model-value="(v: string) => setWhen(index, 'by', v)" />
          </div>
          <div class="flex items-center gap-2">
            <UCheckbox :model-value="trigger.when.cleared !== undefined" label="squad is cleared" @update:model-value="(v) => condition(index, 'cleared', checked(v))" />
            <USelect v-if="trigger.when.cleared !== undefined" :model-value="trigger.when.cleared" :items="squadItems" @update:model-value="(v: string) => setWhen(index, 'cleared', v)" />
          </div>
          <div class="flex items-center gap-2">
            <UCheckbox :model-value="trigger.when.done !== undefined" label="another trigger is done" @update:model-value="(v) => condition(index, 'done', checked(v))" />
            <USelect v-if="trigger.when.done !== undefined" :model-value="trigger.when.done" :items="namedTriggers" @update:model-value="(v: string) => setWhen(index, 'done', v)" />
          </div>
        </div>

        <div class="grid grid-cols-2 gap-2 text-xs">
          <label>Wait (s)<UInput :model-value="trigger.wait ?? 0" type="number" min="0" class="mt-1 w-full" @update:model-value="(v: string | number) => patchTrigger(index, { wait: Number(v) || undefined })" /></label>
          <label>Times (0 = always)<UInput :model-value="trigger.times ?? 1" type="number" min="0" class="mt-1 w-full" @update:model-value="(v: string | number) => patchTrigger(index, { times: Number(v) })" /></label>
        </div>

        <div class="space-y-1">
          <p class="text-xs font-semibold text-muted">Do</p>
          <div v-for="(action, at) in trigger.do" :key="at" class="flex items-center gap-2">
            <USelect
              :model-value="actionKind(action)"
              :items="actionKinds"
              class="w-28"
              @update:model-value="(v: string) => setAction(index, at, makeAction(v, ['spawn', 'remove'].includes(v) ? squadNames[0] ?? '' : ''))"
            />
            <USelect
              v-if="['spawn', 'remove'].includes(actionKind(action))"
              :model-value="actionValue(action)"
              :items="squadItems"
              class="flex-1"
              @update:model-value="(v: string) => setAction(index, at, makeAction(actionKind(action), v))"
            />
            <UInput
              v-else
              :model-value="actionValue(action)"
              :maxlength="200"
              placeholder="text shown to the player"
              class="flex-1"
              @update:model-value="(v: string | number) => setAction(index, at, makeAction(actionKind(action), String(v)))"
            />
            <UButton icon="i-lucide-x" size="xs" color="neutral" variant="ghost" title="Remove action" @click="dropAction(index, at)" />
          </div>
          <UButton label="Add action" size="xs" color="neutral" variant="outline" @click="addAction(index)" />
        </div>
      </li>
    </ol>
    <UButton label="Add trigger" size="sm" color="neutral" variant="outline" block @click="addTrigger" />
  </section>
</template>
