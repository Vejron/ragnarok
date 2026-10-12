import { describe, expect, it } from 'vitest'
import * as THREE from 'three'
import { tyreSlip, wheelLayout } from '@/sim/slip'
import { rasterSurface, Surfaces, SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_WATER } from '@/sim/surface'
import { Skidmarks, SKID_LIFE } from '@/render/skidmarks'
import { Particles } from '@/render/particles'
import { VehicleFx, type FxWorld } from '@/render/tyres'
import { COVER_FARM, COVER_WATER, ROAD_MINOR, ROAD_PATH, NO_WATER } from '@/world'
import { KIND_HELI, KIND_JEEP, KIND_SPORTS, KIND_TANK } from '@/net/codec'

describe('tyreSlip', () => {
  it('grips on a straight cruise and in a gentle turn', () => {
    expect(tyreSlip(0, 20, 0, true)).toBe(0)
    expect(tyreSlip(1.2, 20, 0, true)).toBe(0)
  })

  it('slides when the contact patch moves sideways', () => {
    expect(tyreSlip(4, 20, 0, false)).toBeGreaterThan(0.3)
    expect(tyreSlip(-9, 20, 0, false)).toBe(1)
  })

  it('locks under hard braking and spins on a hard launch', () => {
    expect(tyreSlip(0, 20, -11, false)).toBeGreaterThan(0.3)
    expect(tyreSlip(0, -20, 11, false)).toBeGreaterThan(0.3)
    expect(tyreSlip(0, 2, -11, false)).toBe(0)
    expect(tyreSlip(0, 4, 12, true)).toBeGreaterThan(0.3)
    expect(tyreSlip(0, 4, 12, false)).toBe(0)
    expect(tyreSlip(0, 30, 12, true)).toBe(0)
  })

  it('knows the wheels of every kind but the helicopter', () => {
    expect(wheelLayout(KIND_JEEP)!.contacts).toHaveLength(4)
    expect(wheelLayout(KIND_SPORTS)!.steered).toBe(2)
    expect(wheelLayout(KIND_TANK)!.steered).toBe(0)
    expect(wheelLayout(KIND_HELI)).toBeNull()
    // Front first: the front pair is towards -Z.
    expect(wheelLayout(KIND_JEEP)!.contacts[0]![2]).toBeLessThan(0)
  })
})

describe('surfaces', () => {
  const tile = {
    cover: [
      { kind: COVER_FARM, rings: [[[0, 0], [40, 0], [40, 40], [0, 40]] as [number, number][], [[10, 10], [20, 10], [20, 20], [10, 20]] as [number, number][]] },
      { kind: COVER_WATER, rings: [[[60, 60], [80, 60], [80, 80], [60, 80]] as [number, number][]] },
    ],
    roads: [
      { kind: ROAD_MINOR, width: 6, points: [[0, 50], [100, 50]] as [number, number][] },
      { kind: ROAD_PATH, width: 3, points: [[90, 0], [90, 30]] as [number, number][] },
    ],
    water: new Float32Array(),
  }

  it('paints cover with holes, then roads over it', () => {
    const surface = rasterSurface(tile, 0, 0, 100)
    const at = (x: number, z: number) => surface.cells[Math.floor(z / 2) * surface.side + Math.floor(x / 2)]
    expect(at(5, 5)).toBe(SURFACE_DIRT)
    expect(at(15, 15)).toBe(SURFACE_GRASS)
    expect(at(70, 70)).toBe(SURFACE_WATER)
    expect(at(50, 50)).toBe(SURFACE_ASPHALT)
    expect(at(50, 56)).toBe(SURFACE_GRASS)
    expect(at(90, 15)).toBe(SURFACE_DIRT)
    expect(at(95, 95)).toBe(SURFACE_GRASS)
  })

  it('finds deep water from the water heights', () => {
    const ground = { x0: 0, z0: 0, cell: 50, side: 3, heights: new Float32Array(9) }
    const water = new Float32Array(9).fill(NO_WATER)
    water[4] = 1
    const surface = rasterSurface({ cover: [], roads: [], water }, 0, 0, 100, ground)
    const at = (x: number, z: number) => surface.cells[Math.floor(z / 2) * surface.side + Math.floor(x / 2)]
    expect(at(50, 50)).toBe(SURFACE_WATER)
    expect(at(5, 5)).toBe(SURFACE_GRASS)
  })

  it('looks surfaces up across tiles and forgets unloaded ones', () => {
    const surfaces = new Surfaces(100, 100)
    surfaces.add({ col: 1, row: 1, hash: 0 }, tile, null)
    expect(surfaces.at(5, 5)).toBe(SURFACE_DIRT)
    expect(surfaces.at(-50, -50)).toBe(SURFACE_GRASS)
    surfaces.delete({ col: 1, row: 1, hash: 0 })
    expect(surfaces.at(5, 5)).toBe(SURFACE_GRASS)
  })
})

describe('Skidmarks', () => {
  const side = new THREE.Vector3(1, 0, 0)

  it('joins a wheel’s marks into a strip and starts over after a lift', () => {
    const skids = new Skidmarks(10)
    skids.mark('a', new THREE.Vector3(0, 0, 0), side, 0.3, 0, 1)
    expect(skids.laid).toBe(0)
    skids.mark('a', new THREE.Vector3(0, 0, -0.1), side, 0.3, 0, 1)
    expect(skids.laid).toBe(0)
    skids.mark('a', new THREE.Vector3(0, 0, -1), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
    skids.lift('a')
    skids.mark('a', new THREE.Vector3(0, 0, -2), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
    // A jump is not drawn as a long smear.
    skids.mark('a', new THREE.Vector3(0, 0, -20), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
  })

  it('reuses its oldest quads and fades them out', () => {
    const skids = new Skidmarks(3)
    for (let i = 0; i <= 5; i++) skids.mark('a', new THREE.Vector3(0, 0, -i), side, 0.3, 0, 1)
    expect(skids.laid).toBe(5)
    expect(skids.ageOf(0)).toBe(0)
    skids.update(SKID_LIFE / 2)
    expect(skids.ageOf(0)).toBeCloseTo(0.5)
    skids.update(SKID_LIFE)
    expect(skids.ageOf(2)).toBe(1)
  })
})

describe('Particles', () => {
  it('moves, bounces off the ground and dies', () => {
    const particles = new Particles(4)
    particles.groundAt = () => 0
    particles.spawn({ position: [0, 1, 0], velocity: [1, 0, 0], life: 1, size: [1, 1], color: 0xffffff, alpha: 1, gravity: 9.8, bounce: true })
    particles.spawn({ position: [0, 1, 0], velocity: [0, 0, 0], life: 0.1, size: [1, 1], color: 0xffffff, alpha: 1 })
    for (let i = 0; i < 30; i++) particles.update(1 / 60)
    expect(particles.alive).toBe(1)
    const [x, y] = particles.positionOf(0)
    expect(x).toBeGreaterThan(0.3)
    expect(y).toBeGreaterThanOrEqual(0)
    for (let i = 0; i < 60; i++) particles.update(1 / 60)
    expect(particles.alive).toBe(0)
  })

  it('never holds more than its capacity', () => {
    const particles = new Particles(3)
    for (let i = 0; i < 10; i++) particles.spawn({ position: [0, 0, 0], velocity: [0, 0, 0], life: 1, size: [1, 1], color: 0, alpha: 1 })
    expect(particles.alive).toBe(3)
  })
})

describe('VehicleFx', () => {
  function world(surface: number): FxWorld & { smoke: Particles; skids: Skidmarks } {
    return {
      smoke: new Particles(2000),
      glow: new Particles(100, true),
      skids: new Skidmarks(500),
      groundAt: () => 0,
      surfaceAt: () => surface,
      camera: new THREE.Vector3(0, 5, 10),
    }
  }

  /** A jeep at ride height, moving with `velocity` (m/s) while pointing along -Z, for `frames` frames. */
  function slide(fx: VehicleFx, w: FxWorld, velocity: [number, number], frames: number) {
    const dt = 1 / 60
    const rotation = [0, 0, 0, 1] as const
    for (let i = 0; i < frames; i++) {
      fx.update(w, [velocity[0] * i * dt, 0.857, velocity[1] * i * dt], rotation, 0, true, dt)
      w.smoke.update(dt)
    }
  }

  it('leaves no marks or smoke on a straight cruise on tarmac', () => {
    const w = world(SURFACE_ASPHALT)
    slide(new VehicleFx(1, KIND_JEEP), w, [0, -15], 120)
    expect(w.skids.laid).toBe(0)
  })

  it('marks the tarmac and smokes in a slide', () => {
    const w = world(SURFACE_ASPHALT)
    slide(new VehicleFx(1, KIND_JEEP), w, [8, -15], 120)
    expect(w.skids.laid).toBeGreaterThan(20)
    expect(w.smoke.alive).toBeGreaterThan(10)
  })

  it('ruts soft ground and raises dust even without sliding', () => {
    const w = world(SURFACE_DIRT)
    slide(new VehicleFx(1, KIND_JEEP), w, [0, -15], 120)
    expect(w.skids.laid).toBeGreaterThan(10)
    expect(w.smoke.alive).toBeGreaterThan(0)
  })

  it('splashes in water without marking it', () => {
    const w = world(SURFACE_WATER)
    slide(new VehicleFx(1, KIND_JEEP), w, [0, -10], 120)
    expect(w.skids.laid).toBe(0)
    expect(w.smoke.alive).toBeGreaterThan(5)
  })

  it('reports a landing after a drop, and neither a hop nor a drive', () => {
    const w = world(SURFACE_ASPHALT)
    const fx = new VehicleFx(1, KIND_JEEP)
    const dt = 1 / 60
    const rotation = [0, 0, 0, 1] as const
    // Rolling on the ground, then falling 12 m over a second and a bit, then down again.
    for (let i = 0; i < 20; i++) fx.update(w, [0, 0.857, -i * 0.2], rotation, 0, true, dt)
    expect(fx.takeLanding()).toBe(0)
    expect(fx.grounded).toBe(true)
    for (let i = 0; i < 70; i++) fx.update(w, [0, 3 + (70 - i) * 0.17, -4 - i * 0.2], rotation, 0, true, dt)
    expect(fx.grounded).toBe(false)
    fx.update(w, [0, 0.857, -18], rotation, 0, true, dt)
    expect(fx.grounded).toBe(true)
    expect(fx.takeLanding()).toBeGreaterThan(3)
    expect(fx.takeLanding()).toBe(0)
    // A bump of a few centimetres is not air.
    for (let i = 0; i < 30; i++) fx.update(w, [0, 0.857 + (i % 2) * 0.05, -18 - i * 0.2], rotation, 0, true, dt)
    expect(fx.takeLanding()).toBe(0)
  })

  it('measures the speed it crashed from and what it drives on', () => {
    const w = world(SURFACE_DIRT)
    const fx = new VehicleFx(1, KIND_JEEP)
    slide(fx, w, [0, -20], 60)
    expect(fx.impactSpeed).toBeGreaterThan(15)
    expect(fx.forwardSpeed).toBeGreaterThan(15)
    expect(fx.surfaceUnder).toBe(SURFACE_DIRT)
  })
})
