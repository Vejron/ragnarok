import { describe, expect, it, vi } from 'vitest'
import { quadToUnit, unitToQuad, type Point } from '@/phone/homography'
import { onScreen } from '@/phone/pointer'
import { PhoneScreen, type PhoneView } from '@/phone/screen'
import { APP_H, APP_TOP, SCREEN_H, SCREEN_W } from '@/phone/ui'
import { MapTiles } from '@/hud/maptiles'
import { PHONE_ACCEPT, PHONE_CANCEL, PHONE_ORDER, JOB_FETCH, DEALER_KINDS, DEALER_PRICES } from '@/sim/consts'
import type { DeliveryView } from '@/deliveries'

/** A 2D context that draws nothing and measures every character 10 px wide. */
function fakeContext(): CanvasRenderingContext2D {
  return new Proxy({} as Record<string | symbol, unknown>, {
    get(target, key) {
      if (key in target) return target[key]
      if (key === 'measureText') return (text: string) => ({ width: text.length * 10 })
      if (key === 'getTransform') return () => ({ a: 1 })
      return () => {}
    },
    set(target, key, value) {
      target[key] = value
      return true
    },
  }) as unknown as CanvasRenderingContext2D
}

const square: [Point, Point, Point, Point] = [[0.2, 0.1], [0.6, 0.1], [0.6, 0.9], [0.2, 0.9]]
const tilted: [Point, Point, Point, Point] = [[0.42, 0.3], [0.61, 0.32], [0.64, 0.85], [0.38, 0.83]]

describe('the phone screen on the page', () => {
  it('maps its corners and middle to the screen\'s own corners and middle', () => {
    for (const corners of [square, tilted]) {
      const want: Point[] = [[0, 0], [1, 0], [1, 1], [0, 1]]
      corners.forEach((corner, i) => {
        const uv = quadToUnit(corners, corner)!
        expect(uv[0]).toBeCloseTo(want[i]![0], 6)
        expect(uv[1]).toBeCloseTo(want[i]![1], 6)
      })
      const back = quadToUnit(corners, unitToQuad(corners, [0.3, 0.7]))!
      expect(back[0]).toBeCloseTo(0.3, 6)
      expect(back[1]).toBeCloseTo(0.7, 6)
    }
    const middle = quadToUnit(square, [0.4, 0.5])!
    expect(middle[0]).toBeCloseTo(0.5, 9)
    expect(middle[1]).toBeCloseTo(0.5, 9)
  })

  it('turns a point of the page into the screen\'s pixels, and nothing off it', () => {
    const [x, y] = onScreen(square, [0.4, 0.5])!
    expect(x).toBeCloseTo(SCREEN_W / 2, 6)
    expect(y).toBeCloseTo(SCREEN_H / 2, 6)
    expect(onScreen(square, [0.1, 0.5])).toBeNull()
    expect(onScreen(null, [0.4, 0.5])).toBeNull()
  })
})

describe('PhoneScreen', () => {
  const tiles = new MapTiles()
  const marks = { x: 100, z: -50, yaw: 0, blips: [], circle: null, pulse: 0 }
  const delivery: DeliveryView = {
    job: null,
    target: null,
    offers: [{ id: 7, shopX: 120, shopZ: -40, houseX: 600, houseZ: 200, pay: 63, par: 200 }],
    mine: [],
    cash: 40,
  }
  const make = () => {
    const actions = { phone: vi.fn(), open: vi.fn() }
    const screen = new PhoneScreen(actions)
    return { screen, actions }
  }
  const tap = (screen: PhoneScreen, x: number, y: number) => {
    screen.pointerDown(x, y)
    screen.pointerUp(x, y)
  }
  const away = { cash: 40, distance: 300, near: false }
  const view = (app: PhoneView['app'], d: DeliveryView | null = delivery, dealer = away): PhoneView => ({ app, tiles, marks, delivery: d, dealer, clock: '12:00' })

  it('switches app by its tabs', () => {
    const { screen, actions } = make()
    screen.draw(fakeContext(), view('map'))
    tap(screen, SCREEN_W * 0.5, SCREEN_H - 60)
    expect(actions.open).toHaveBeenCalledWith('jobs')
    tap(screen, SCREEN_W * 0.85, SCREEN_H - 60)
    expect(actions.open).toHaveBeenLastCalledWith('dealer')
    tap(screen, SCREEN_W * 0.15, SCREEN_H - 60)
    expect(actions.open).toHaveBeenLastCalledWith('map')
  })

  it('buys a vehicle at a dealer when its price is tapped, and only what the cash covers', () => {
    const { screen, actions } = make()
    // The first row's price button: right of the list, under the header and the note.
    const row = (i: number) => APP_TOP + 104 + 16 + 72 + i * (104 + 10) + 52
    screen.draw(fakeContext(), view('dealer', delivery, away))
    tap(screen, SCREEN_W - 16 - 24 - 75, row(0))
    expect(actions.phone).not.toHaveBeenCalled()
    screen.draw(fakeContext(), view('dealer', delivery, { cash: DEALER_PRICES[1], distance: 5, near: true }))
    tap(screen, SCREEN_W - 16 - 24 - 75, row(0))
    expect(actions.phone).toHaveBeenCalledWith(PHONE_ORDER, DEALER_KINDS[0])
    // The third costs more than the cash.
    tap(screen, SCREEN_W - 16 - 24 - 75, row(2))
    expect(actions.phone).toHaveBeenCalledTimes(1)
  })

  it('takes an order when its price is tapped', () => {
    const { screen, actions } = make()
    screen.draw(fakeContext(), view('jobs'))
    // The first order's price button: right of the list, under the header and the heading.
    tap(screen, SCREEN_W - 110, APP_TOP + 104 + 16 + 52 + 75)
    expect(actions.phone).toHaveBeenCalledWith(PHONE_ACCEPT, 7)
  })

  it('gives back an order not yet fetched, and takes no other meanwhile', () => {
    const { screen, actions } = make()
    const mine = { ...delivery, mine: [{ id: 3, stage: JOB_FETCH, x: 120, z: -40, pay: 50, left: 200 }] }
    screen.draw(fakeContext(), view('jobs', mine))
    // Cancel, at the bottom right of the job's card.
    tap(screen, SCREEN_W - 16 - 24 - 75, APP_TOP + 104 + 16 + 150 - 24 - 26)
    expect(actions.phone).toHaveBeenCalledWith(PHONE_CANCEL, 3)
    // The order below can't be taken while one is to be fetched.
    tap(screen, SCREEN_W - 110, APP_TOP + 104 + 16 + 150 + 12 + 52 + 75)
    expect(actions.phone).toHaveBeenCalledTimes(1)
  })

  it('pans the map by a drag, and a drag taps nothing', () => {
    const { screen, actions } = make()
    screen.draw(fakeContext(), view('map'))
    const perMetre = screen.map.perMetre
    screen.pointerDown(270, 500)
    screen.pointerMove(300, 500)
    screen.pointerMove(370, 500)
    expect(screen.dragging).toBe(true)
    screen.pointerUp(370, 500)
    expect(screen.map.follow).toBe(false)
    expect(screen.map.centre[0]).toBeCloseTo(100 - 100 / perMetre)
    expect(actions.open).not.toHaveBeenCalled()
  })

  it('zooms the map by the wheel, and follows the reader again from its button', () => {
    const { screen } = make()
    screen.draw(fakeContext(), view('map'))
    const before = screen.map.perMetre
    screen.wheel(-1)
    expect(screen.map.perMetre).toBeGreaterThan(before)
    screen.map.pan(10, 0, marks)
    screen.draw(fakeContext(), view('map'))
    // The centre button sits under the zoom buttons, up the right side.
    tap(screen, SCREEN_W - 24 - 36, APP_TOP + APP_H * 0.36 + 168 + 36)
    expect(screen.map.follow).toBe(true)
  })
})
