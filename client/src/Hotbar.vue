<script setup lang="ts">
import { HOTBAR_SLOTS, itemDef, type ItemSlot } from './sim/items'

defineProps<{
  slots: readonly ItemSlot[]
  selected: number
  /** How far a reload or a bandage has got, 0..1; 0 when idle. */
  progress: number
  /** Slots can be tapped. */
  touch: boolean
}>()
defineEmits<{ select: [slot: number] }>()

const label = (slot: ItemSlot) => itemDef(slot.item)?.name ?? ''
/** `ammo` for a gun, the stack size for a thing you use up. */
const detail = (slot: ItemSlot) => {
  const item = itemDef(slot.item)
  if (!item) return ''
  if (item.kind === 'gun') return `${slot.ammo}/${item.mag}`
  return slot.count > 1 ? `×${slot.count}` : ''
}
</script>

<template>
  <div class="hotbar" :class="{ touch }">
    <div v-if="progress > 0" class="progress"><div class="fill" :style="{ width: 100 * progress + '%' }" /></div>
    <button
      v-for="n in HOTBAR_SLOTS"
      :key="n"
      type="button"
      class="slot"
      :class="{ selected: selected === n - 1, empty: !slots[n - 1] || slots[n - 1]!.item === 0, dry: slots[n - 1]?.item && itemDef(slots[n - 1]!.item)?.kind === 'gun' && slots[n - 1]!.ammo === 0 }"
      @pointerdown.prevent="$emit('select', n - 1)"
    >
      <span class="key">{{ n }}</span>
      <span class="name">{{ slots[n - 1] ? label(slots[n - 1]!) : '' }}</span>
      <span class="detail">{{ slots[n - 1] ? detail(slots[n - 1]!) : '' }}</span>
    </button>
  </div>
</template>

<style scoped>
.hotbar {
  position: fixed; left: 50%; bottom: 16px; transform: translateX(-50%); display: flex; gap: 6px; pointer-events: none;
  font-family: ui-rounded, 'SF Pro Rounded', system-ui, sans-serif;
}
.hotbar.touch { bottom: max(12px, env(safe-area-inset-bottom)); gap: 4px; }
.hotbar.touch .slot { pointer-events: auto; width: 54px; height: 54px; }
.slot {
  position: relative; width: 68px; height: 68px; border: 0; border-radius: 12px; padding: 0; display: grid; place-content: center;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25); color: #1d2b36; text-align: center;
  touch-action: none; -webkit-tap-highlight-color: transparent;
}
.slot.selected { background: rgba(255, 255, 255, 0.92); box-shadow: 0 0 0 3px #ffbe0b, 0 0 0 5px rgba(0, 0, 0, 0.25); transform: translateY(-4px); }
.slot.empty { opacity: 0.7; }
.key { position: absolute; top: 3px; left: 6px; font-size: 11px; font-weight: 800; opacity: 0.6; }
.name { font-size: 12px; font-weight: 800; }
.detail { font-size: 12px; font-weight: 700; opacity: 0.8; }
.slot.dry .detail { color: #b3261e; opacity: 1; }
.progress {
  position: absolute; left: 0; right: 0; top: -14px; height: 8px; border-radius: 999px; overflow: hidden;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 2px rgba(0, 0, 0, 0.25);
}
.fill { height: 100%; background: #ffbe0b; }
</style>
