import { describe, expect, it } from 'vitest'
import { aimSpeedScale, ITEM_AK5, ITEM_GRENADE, ITEM_SNIPER } from '@/sim/items'
import { BUTTON_ALT, BUTTON_RUN, RUN_SPEED, STANDING_Y, TICK_DT, TICK_RATE } from '@/sim/consts'
import { flatMotor } from './worlds'

// The movement rules themselves are the server's (`crates/shared/src/movement.rs`, tested there) and
// run here through the WASM build; what is left to test on this side is what the client feeds them.

const frame = (seq: number, forward: number, buttons = 0) => ({ seq, strafe: 0, forward, yaw: 0, buttons, pitch: 0, view: 0, slot: 0 })
const standing = () => ({ position: [0, STANDING_Y, 0] as [number, number, number], yaw: 0, vy: 0, grounded: true, knock: [0, 0] as [number, number], ride: 0, prone: false, chute: false, spent: 0, winded: false, lost: 0 })

describe('movement', () => {
  it('walks slower by the speed scale', async () => {
    const motor = await flatMotor()
    let full = standing()
    let half = standing()
    for (let seq = 1; seq <= TICK_RATE; seq++) {
      full = motor.step(full, frame(seq, 127, BUTTON_RUN), TICK_DT, 1)
      half = motor.step(half, frame(seq, 127, BUTTON_RUN), TICK_DT, 0.5)
    }
    expect(full.position[2]).toBeCloseTo(-RUN_SPEED, 1)
    expect(half.position[2]).toBeCloseTo(-RUN_SPEED * 0.5, 1)
  })

  it('only a gun with aim held slows its holder', () => {
    expect(aimSpeedScale(ITEM_AK5, 0)).toBe(1)
    expect(aimSpeedScale(ITEM_AK5, BUTTON_ALT)).toBeLessThan(1)
    expect(aimSpeedScale(ITEM_SNIPER, BUTTON_ALT)).toBeLessThan(aimSpeedScale(ITEM_AK5, BUTTON_ALT))
    expect(aimSpeedScale(ITEM_GRENADE, BUTTON_ALT)).toBe(1)
  })
})
