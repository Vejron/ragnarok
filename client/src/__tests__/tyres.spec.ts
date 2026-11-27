import { Vec3, Vector3 } from '@ragnarok/engine'
import { afterEach, describe, expect, it, vi } from 'vitest'
import { kindView } from '@/kinds'
import { tyreSlip } from '@/sim/slip'
import { rasterSurface, Surfaces, SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_GRASS, SURFACE_WATER } from '@/sim/surface'
import { Skidmarks, SKID_LIFE } from '@/render/skidmarks'
import { grassMask } from '@/render/grass'
import { Particles } from '@/render/particles'
import { VehicleFx, type FxWorld } from '@/render/tyres'
import { JetSpray } from '@/render/jetspray'
import { COVER_ASPHALT, COVER_FARM, COVER_WATER, RAIL_TRAIN, RAIL_TRAM, ROAD_MINOR, ROAD_PATH, NO_WATER } from '@/world'
import { VOLVO_740_RIDE_HEIGHT } from '@/sim/consts'
import { KIND_CB90, KIND_HELI, KIND_JEEP, KIND_VOLVO_740, KIND_PLANE, KIND_RAFT, KIND_SNOWMOBILE, KIND_SPORTS, KIND_TANK } from '@/net/codec'
import { setIced, setWinter } from '@/render/season'
import { JEEP_RIDE_HEIGHT, SNOWMOBILE_RIDE_HEIGHT, TANK_HALF_EXTENTS, TANK_RIDE_HEIGHT } from '@/sim/consts'
import { SnowRoost } from '@/render/snowroost'

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
    expect(kindView(KIND_JEEP).fx.wheels!.contacts).toHaveLength(4)
    expect(kindView(KIND_SPORTS).fx.wheels!.steered).toBe(2)
    expect(kindView(KIND_TANK).fx.wheels!.steered).toBe(0)
    expect(kindView(KIND_HELI).fx.wheels).toBeNull()
    // A plane steers its main wheels and drags its tail skid.
    expect(kindView(KIND_PLANE).fx.wheels!.steered).toBe(2)
    expect(kindView(KIND_PLANE).fx.wheels!.contacts[2]![2]).toBeGreaterThan(2)
    // Front first: the front pair is towards -Z.
    expect(kindView(KIND_JEEP).fx.wheels!.contacts[0]![2]).toBeLessThan(0)
  })
})

describe('surfaces', () => {
  const tile = {
    cover: [
      { kind: COVER_FARM, rings: [[[0, 0], [40, 0], [40, 40], [0, 40]] as [number, number][], [[10, 10], [20, 10], [20, 20], [10, 20]] as [number, number][]] },
      { kind: COVER_WATER, rings: [[[60, 60], [80, 60], [80, 80], [60, 80]] as [number, number][]] },
    ],
    roads: [
      { kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, oneWay: false, points: [[0, 50], [100, 50]] as [number, number][] },
      { kind: ROAD_PATH, width: 3, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, oneWay: false, points: [[90, 0], [90, 30]] as [number, number][] },
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

  it('paints a car park as asphalt, and nothing grows on it', () => {
    const lot = { cover: [{ kind: COVER_ASPHALT, rings: [[[20, 20], [60, 20], [60, 60], [20, 60]] as [number, number][]] }], roads: [], water: new Float32Array(), buildings: [], streams: [] }
    const surface = rasterSurface(lot, 0, 0, 100)
    const at = (x: number, z: number) => surface.cells[Math.floor(z / 2) * surface.side + Math.floor(x / 2)]
    expect(at(40, 40)).toBe(SURFACE_ASPHALT)
    expect(at(80, 80)).toBe(SURFACE_GRASS)
    const mask = grassMask(lot, 0, 0, 100)
    expect(mask.cells[Math.floor(40 / 2) * mask.side + Math.floor(40 / 2)]).toBe(0)
    expect(mask.cells[Math.floor(80 / 2) * mask.side + Math.floor(80 / 2)]).toBe(1)
  })

  it('paints ballast as gravel under the roads, and not under a tram', () => {
    const rails = [
      { kind: RAIL_TRAIN, gauge: 1.435, points: [[20, 0], [20, 100]] as [number, number][], deck: null },
      { kind: RAIL_TRAM, gauge: 1.435, points: [[60, 0], [60, 40]] as [number, number][], deck: null },
    ]
    const surface = rasterSurface({ cover: [], roads: [{ kind: ROAD_MINOR, width: 6, carried: false, lanes: 0, forward: 0, marks: 0, along: 0, oneWay: false, points: [[0, 50], [100, 50]] }], water: new Float32Array(), rails }, 0, 0, 100)
    const at = (x: number, z: number) => surface.cells[Math.floor(z / 2) * surface.side + Math.floor(x / 2)]
    expect(at(20, 20)).toBe(SURFACE_DIRT)
    expect(at(26, 20)).toBe(SURFACE_GRASS)
    expect(at(20, 50)).toBe(SURFACE_ASPHALT)
    expect(at(60, 20)).toBe(SURFACE_GRASS)
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
  const side = new Vector3(1, 0, 0)

  it('joins a wheel’s marks into a strip and starts over after a lift', () => {
    const skids = new Skidmarks(10)
    skids.mark('a', new Vector3(0, 0, 0), side, 0.3, 0, 1)
    expect(skids.laid).toBe(0)
    skids.mark('a', new Vector3(0, 0, -0.1), side, 0.3, 0, 1)
    expect(skids.laid).toBe(0)
    skids.mark('a', new Vector3(0, 0, -1), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
    skids.lift('a')
    skids.mark('a', new Vector3(0, 0, -2), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
    // A jump is not drawn as a long smear.
    skids.mark('a', new Vector3(0, 0, -20), side, 0.3, 0, 1)
    expect(skids.laid).toBe(1)
  })

  it('reuses its oldest quads and fades them out', () => {
    const skids = new Skidmarks(3)
    for (let i = 0; i <= 5; i++) skids.mark('a', new Vector3(0, 0, -i), side, 0.3, 0, 1)
    expect(skids.laid).toBe(5)
    expect(skids.ageOf(0)).toBe(0)
    skids.update(SKID_LIFE / 2)
    expect(skids.ageOf(0)).toBeCloseTo(0.5)
    skids.update(SKID_LIFE)
    expect(skids.ageOf(2)).toBe(1)
  })
})

describe('Particles', () => {
  it('counts what it has spawned until it dies', () => {
    const particles = new Particles(4)
    particles.groundAt = () => 0
    particles.spawn({ position: [0, 1, 0], velocity: [1, 0, 0], life: 1, size: [1, 1], color: 0xffffff, alpha: 1, gravity: 9.8, bounce: true })
    particles.spawn({ position: [0, 1, 0], velocity: [0, 0, 0], life: 0.1, size: [1, 1], color: 0xffffff, alpha: 1 })
    // Where they go is the GPU's business (the engine's particle example shows it); the pool counts them.
    for (let i = 0; i < 30; i++) particles.update(1 / 60)
    expect(particles.alive).toBe(1)
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
  afterEach(() => vi.restoreAllMocks())

  function world(surface: number): FxWorld & { smoke: Particles; skids: Skidmarks } {
    return {
      smoke: new Particles(2000),
      glow: new Particles(100, true), halo: new Particles(64, true, true),
      skids: new Skidmarks(500),
      groundAt: () => 0,
      surfaceAt: () => surface,
      camera: new Vec3(0, 5, 10),
    }
  }

  /** A jeep at ride height, moving with `velocity` (m/s) while pointing along -Z, for `frames` frames; unpowered, it puffs no exhaust. */
  function slide(fx: VehicleFx, w: FxWorld, velocity: [number, number], frames: number, powered = true, ride = 0.857) {
    const dt = 1 / 60
    const rotation = [0, 0, 0, 1] as const
    for (let i = 0; i < frames; i++) {
      fx.update(w, [velocity[0] * i * dt, ride, velocity[1] * i * dt], rotation, 0, powered, dt)
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

  it('smokes and marks the tarmac under a rear-driven car spinning its tyres at a crawl, and not without the spin', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.01)
    const w = world(SURFACE_ASPHALT)
    const fx = new VehicleFx(1, KIND_VOLVO_740)
    fx.spin = 1
    slide(fx, w, [0, -1], 120, true, VOLVO_740_RIDE_HEIGHT)
    expect(w.skids.laid).toBeGreaterThan(5)
    expect(w.smoke.alive).toBeGreaterThan(10)
    const still = world(SURFACE_ASPHALT)
    slide(new VehicleFx(1, KIND_VOLVO_740), still, [0, -1], 120, true, VOLVO_740_RIDE_HEIGHT)
    expect(still.skids.laid).toBe(0)
  })

  it('ruts soft ground but raises no dust rolling straight along it', () => {
    const w = world(SURFACE_DIRT)
    slide(new VehicleFx(1, KIND_JEEP), w, [0, -15], 120, false)
    expect(w.skids.laid).toBeGreaterThan(10)
    expect(w.smoke.alive).toBe(0)
  })

  it('raises no dust or grit off a parked car whose drawn place jitters by a centimetre or two', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.01)
    const w = world(SURFACE_DIRT)
    const fx = new VehicleFx(1, KIND_JEEP)
    const dt = 1 / 120
    for (let i = 0; i < 240; i++) {
      const jitter = ((i * 7) % 5) * 0.005 - 0.01
      const yaw = ((i * 3) % 4) * 0.002 - 0.003
      // Nobody in it, so no exhaust: anything in the air would be off its tyres.
      fx.update(w, [jitter, 0.857, -jitter], [0, Math.sin(yaw / 2), 0, Math.cos(yaw / 2)], 0, false, dt)
    }
    expect(fx.slip).toBeLessThan(0.05)
    expect(w.smoke.alive).toBe(0)
    expect(w.skids.laid).toBe(0)
  })

  it('raises no dust coasting down from speed, nor pulling away gently', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.01)
    const dt = 1 / 120
    // Let go at 35 m/s, slowing at 2 m/s² to a stop; then setting off at 5 m/s².
    for (const [from, accel] of [[35, -2], [0, 5]] as const) {
      const w = world(SURFACE_DIRT)
      const fx = new VehicleFx(1, KIND_SPORTS)
      for (let i = 0; i < 120 * 6; i++) {
        const t = i * dt
        const stop = accel < 0 ? Math.min(t, from / -accel) : t
        fx.update(w, [0, 0.42, -(from * stop + 0.5 * accel * stop * stop)], [0, 0, 0, 1], 0, false, dt)
      }
      expect(fx.slip, `${from} m/s at ${accel} m/s²`).toBeLessThan(0.05)
      expect(w.smoke.alive).toBe(0)
    }
  })

  it('throws dust off the dirt in a hard turn and pulling away hard', () => {
    vi.spyOn(Math, 'random').mockReturnValue(0.01)
    const turning = world(SURFACE_DIRT)
    slide(new VehicleFx(1, KIND_JEEP), turning, [8, -15], 60, false)
    expect(turning.smoke.alive).toBeGreaterThan(5)

    const launching = world(SURFACE_DIRT)
    const fx = new VehicleFx(1, KIND_JEEP)
    const dt = 1 / 60
    // From a stop at 12 m/s².
    for (let i = 0; i < 60; i++) fx.update(launching, [0, 0.857, -6 * (i * dt) ** 2], [0, 0, 0, 1], 0, false, dt)
    expect(launching.smoke.alive).toBeGreaterThan(5)
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

describe('a boat\'s spray', () => {
  /**
   * A boat of `kind` (a Stridsbåt 90 unless said) afloat heading -Z from `speed` m/s, changing speed at
   * `accel` m/s², for a second: the water it has in the air at the end, all of it and the mist alone.
   */
  function run(speed: number, accel: number, kind = KIND_CB90, bob = 0) {
    const glow = new Particles(2000, true)
    const jetSpray = new JetSpray(glow)
    const w: FxWorld & { smoke: Particles; skids: Skidmarks } = {
      smoke: new Particles(4000),
      glow,
      halo: new Particles(64, true, true),
      skids: new Skidmarks(500),
      groundAt: () => -5,
      surfaceAt: () => SURFACE_WATER,
      waterAt: () => 0,
      camera: new Vec3(0, 5, 10),
      jetSpray,
    }
    const fx = new VehicleFx(1, kind)
    const dt = 1 / 60
    for (let i = 0; i < 60; i++) {
      const t = i * dt
      // Bobbing `bob` metres up and down twice a second, as over a rapid's waves.
      fx.update(w, [0, 0.2 + bob * Math.sin(4 * Math.PI * t), -(speed * t + 0.5 * accel * t * t)], [0, 0, 0, 1], 0, true, dt)
      jetSpray.update(dt)
    }
    return { water: jetSpray.alive, mist: jetSpray.misty }
  }

  it('is a rooster tail of streaks, drops and mist off a waterjet boat under way, none lying still, and no more in a crash stop', () => {
    expect(run(0, 0)).toEqual({ water: 0, mist: 0 })
    const cruising = run(20, 0)
    // About a thousand drops a second in the rooster tail, and a cloud behind.
    expect(cruising.water).toBeGreaterThan(500)
    expect(cruising.mist).toBeGreaterThan(20)
    // Stopping, its tail dies down with its speed: no storm of water thrown forward.
    const stopping = run(20, -10)
    expect(stopping.water).toBeLessThan(cruising.water)
  })

  it('is a low fan off the raft\'s outboard under way, far less than a rooster tail, and more as it slaps over waves', () => {
    expect(run(0, 0, KIND_RAFT)).toEqual({ water: 0, mist: 0 })
    const smooth = run(12, 0, KIND_RAFT)
    expect(smooth.water).toBeGreaterThan(100)
    expect(smooth.water).toBeLessThan(run(20, 0).water / 2)
    expect(run(12, 0, KIND_RAFT, 0.15).water).toBeGreaterThan(smooth.water * 1.2)
  })
})

describe('snow off the tracks', () => {
  function winterWorld(surface: number) {
    const glow = new Particles(500, true)
    const roost = new SnowRoost(glow, () => 0)
    const w: FxWorld & { smoke: Particles; skids: Skidmarks } = {
      smoke: new Particles(2000),
      glow,
      halo: new Particles(64, true, true),
      skids: new Skidmarks(500),
      groundAt: () => 0,
      surfaceAt: () => surface,
      camera: new Vec3(0, 5, 10),
      roost,
    }
    return { w, roost }
  }

  /** `kind` at `ride` height pulling away along -Z at `accel` m/s² from `speed`, for a second. */
  function pull(kind: number, w: FxWorld, ride: number, speed: number, accel: number, powered = true) {
    const fx = new VehicleFx(1, kind)
    const dt = 1 / 60
    for (let i = 0; i < 60; i++) {
      const t = i * dt
      fx.update(w, [0, ride, -(speed * t + 0.5 * accel * t * t)], [0, 0, 0, 1], 0, powered, dt)
    }
  }

  it('throws a plume behind a snowmobile pulling hard, and none behind one rolling along', () => {
    const hard = winterWorld(SURFACE_GRASS)
    pull(KIND_SNOWMOBILE, hard.w, 0.56, 0, 12)
    expect(hard.roost.alive).toBeGreaterThan(80)
    const easy = winterWorld(SURFACE_GRASS)
    pull(KIND_SNOWMOBILE, easy.w, 0.56, 5, 0)
    expect(easy.roost.alive).toBe(0)
    const parked = winterWorld(SURFACE_GRASS)
    pull(KIND_SNOWMOBILE, parked.w, 0.56, 0, 12, false)
    expect(parked.roost.alive).toBe(0)
  })

  it('throws little off a ploughed road', () => {
    const snow = winterWorld(SURFACE_GRASS)
    pull(KIND_SNOWMOBILE, snow.w, 0.56, 0, 12)
    const road = winterWorld(SURFACE_ASPHALT)
    pull(KIND_SNOWMOBILE, road.w, 0.56, 0, 12)
    expect(road.roost.alive).toBeLessThan(snow.roost.alive * 0.4)
  })

  it('washes snow off both tracks of a tank pulling away', () => {
    const { w, roost } = winterWorld(SURFACE_GRASS)
    pull(KIND_TANK, w, 1.13, 0, 3)
    expect(roost.alive).toBeGreaterThan(60)
  })

  it('treats a frozen lake as ice: no spray and no marks', () => {
    setWinter(true)
    setIced(true)
    try {
      const { w } = winterWorld(SURFACE_WATER)
      // Nobody at the wheel, so no exhaust: anything in the air would be spray.
      pull(KIND_JEEP, w, 0.857, 10, 0, false)
      expect(w.smoke.alive).toBe(0)
      expect(w.skids.laid).toBe(0)
    } finally {
      setWinter(false)
      setIced(false)
    }
  })
})

describe('skimming over the water', () => {
  /** `kind` with its wheels' bottoms at the surface of deep water, at `speed` m/s for a second: the water it has in the air. */
  function skim(kind: number, ride: number, speed: number) {
    const glow = new Particles(2000, true)
    const jetSpray = new JetSpray(glow)
    const w: FxWorld = {
      smoke: new Particles(2000), glow, halo: new Particles(64, true, true), skids: new Skidmarks(500),
      groundAt: () => -5, surfaceAt: () => SURFACE_WATER, waterAt: () => 0,
      camera: new Vec3(0, 5, 10), jetSpray,
    }
    const fx = new VehicleFx(1, kind)
    const dt = 1 / 60
    for (let i = 0; i < 60; i++) {
      fx.update(w, [0, ride, -speed * i * dt], [0, 0, 0, 1], 0, true, dt)
      jetSpray.update(dt)
    }
    return jetSpray.alive
  }

  it('throws spray off a car skimming fast, more the faster, and none off one not moving', () => {
    expect(skim(KIND_JEEP, JEEP_RIDE_HEIGHT, 0)).toBe(0)
    const fast = skim(KIND_JEEP, JEEP_RIDE_HEIGHT, 22)
    expect(fast).toBeGreaterThan(100)
    expect(fast).toBeGreaterThan(skim(KIND_JEEP, JEEP_RIDE_HEIGHT, 10))
  })

  it('throws a rooster tail off a snowmobile on a winter world\'s open water', () => {
    setWinter(true)
    try {
      expect(skim(KIND_SNOWMOBILE, SNOWMOBILE_RIDE_HEIGHT, 15)).toBeGreaterThan(skim(KIND_JEEP, JEEP_RIDE_HEIGHT, 15))
    } finally {
      setWinter(false)
    }
  })
})

describe('anything going into the water', () => {
  /** A world whose water, 0.8 m deep, starts 10 m north (-Z) of the origin, recording what stirs it. */
  function lake() {
    const splashes: { radius: number; depth: number }[] = []
    const hulls: string[] = []
    const inLake = (z: number) => z < -10
    const w: FxWorld & { smoke: Particles } = {
      smoke: new Particles(4000),
      glow: new Particles(100, true), halo: new Particles(64, true, true),
      skids: new Skidmarks(500),
      groundAt: () => 0,
      surfaceAt: (_x, z) => (inLake(z) ? SURFACE_WATER : SURFACE_GRASS),
      waterAt: (_x, z) => (inLake(z) ? 0.8 : NO_WATER),
      waves: { hull: (key) => hulls.push(key), splash: (_x, _z, radius, depth) => splashes.push({ radius, depth }) },
      camera: new Vec3(0, 5, 10),
    }
    return { w, splashes, hulls }
  }

  /** `kind` driven north at `speed` m/s for two seconds from `z0`, at height `y` metres. */
  function drive(w: FxWorld, kind: number, z0: number, y: number, speed: number) {
    const fx = new VehicleFx(1, kind)
    const dt = 1 / 60
    for (let i = 0; i < 120; i++) fx.update(w, [0, y, z0 - speed * i * dt], [0, 0, 0, 1], 0, true, dt)
  }

  afterEach(() => vi.restoreAllMocks())

  it('splashes once as big as the tank where it drives in, then pushes a wake and throws water off its bow', () => {
    // Every chance of throwing water off the bow comes up.
    vi.spyOn(Math, 'random').mockReturnValue(0.01)
    const { w, splashes, hulls } = lake()
    drive(w, KIND_TANK, 0, TANK_RIDE_HEIGHT, 8)
    const big = splashes.filter((s) => s.radius >= TANK_HALF_EXTENTS[0])
    expect(big).toHaveLength(1)
    expect(big[0]!.depth).toBeGreaterThan(0.3)
    expect(splashes.length).toBeGreaterThan(10)
    expect(hulls).toContain('vehicle 1')
  })

  it('does not splash when first seen already in the water', () => {
    const { w, splashes } = lake()
    drive(w, KIND_TANK, -20, TANK_RIDE_HEIGHT, 4)
    expect(splashes.filter((s) => s.radius >= TANK_HALF_EXTENTS[0])).toHaveLength(0)
  })

  it('splashes where a helicopter drops into it', () => {
    const { w, splashes } = lake()
    const fx = new VehicleFx(1, KIND_HELI)
    const dt = 1 / 60
    for (let i = 0; i < 60; i++) fx.update(w, [0, 6 - i * 0.12, -20], [0, 0, 0, 1], 0, false, dt)
    expect(splashes.some((s) => s.radius >= 1 && s.depth > 0.3)).toBe(true)
  })
})
