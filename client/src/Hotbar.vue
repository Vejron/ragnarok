<script setup lang="ts">
import { HOTBAR_SLOTS, itemDef, type ItemSlot } from './sim/items'
import { HOTBAR_ICONS } from './hotbarIcons'

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
      :title="slots[n - 1] ? label(slots[n - 1]!) : ''"
      @pointerdown.prevent="$emit('select', n - 1)"
    >
      <span class="key">{{ n }}</span>
      <svg v-if="slots[n - 1] && HOTBAR_ICONS[slots[n - 1]!.item]" class="icon" viewBox="0 0 24 24" v-html="HOTBAR_ICONS[slots[n - 1]!.item]" />
      <span class="detail">{{ slots[n - 1] ? detail(slots[n - 1]!) : '' }}</span>
    </button>
  </div>
</template>

<style scoped>
.hotbar {
  position: fixed; left: 50%; bottom: 12px; transform: translateX(-50%); display: flex; gap: 4px; pointer-events: none;
  font-family: ui-rounded, 'SF Pro Rounded', system-ui, sans-serif;
}
.hotbar.touch { bottom: max(10px, env(safe-area-inset-bottom)); }
.hotbar.touch .slot { pointer-events: auto; }
.slot {
  position: relative; width: 32px; height: 32px; border: 0; border-radius: 6px; padding: 0; display: grid; place-content: center;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.25); color: #1d2b36;
  touch-action: none; -webkit-tap-highlight-color: transparent;
}
.slot.selected { background: rgba(255, 255, 255, 0.92); box-shadow: 0 0 0 2px #ffbe0b, 0 0 0 3px rgba(0, 0, 0, 0.25); transform: translateY(-2px); }
.slot.empty { opacity: 0.6; }
.icon { width: 24px; height: 24px; }
.key { position: absolute; top: 1px; left: 3px; font-size: 8px; font-weight: 800; opacity: 0.6; line-height: 1; }
.detail { position: absolute; right: 2px; bottom: 0; font-size: 9px; font-weight: 800; line-height: 1; text-shadow: 0 0 2px #fff, 0 0 2px #fff; }
.slot.dry .detail { color: #b3261e; }
.slot.dry .icon { opacity: 0.45; }
.progress {
  position: absolute; left: 0; right: 0; top: -8px; height: 5px; border-radius: 999px; overflow: hidden;
  background: rgba(255, 255, 255, 0.55); box-shadow: 0 0 0 1px rgba(0, 0, 0, 0.25);
}
.fill { height: 100%; background: #ffbe0b; }
</style>
