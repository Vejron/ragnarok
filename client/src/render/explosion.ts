import * as THREE from 'three'
import { SURFACE_ASPHALT, SURFACE_DIRT, SURFACE_SAND, SURFACE_WATER } from '@/sim/surface'
import type { Debris } from './debris'
import type { FxWorld } from './tyres'

/** How much of each layer a kind of blast has; 1 is a grenade-sized one. */
export interface ExplosionProfile {
  /** Fireball and flame. */
  fire: number
  /** Sparks and embers thrown out. */
  sparks: number
  /** Smoke column that rises and lingers. */
  smoke: number
  /** Dirt thrown up and the dust ring along the ground. */
  dirt: number
  /** Solid chunks that tumble, drawn as meshes. */
  chunks: number
  /** Dark, greasy smoke and metal rather than dirt. */
  wreck?: boolean
}

export const BLASTS = {
  grenade: { fire: 1, sparks: 1, smoke: 1, dirt: 0.8, chunks: 0.5 },
  rocket: { fire: 1, sparks: 1.2, smoke: 1.2, dirt: 1, chunks: 0.7 },
  shell: { fire: 1.3, sparks: 1.3, smoke: 1.5, dirt: 1.6, chunks: 1.2 },
  cannon: { fire: 0.6, sparks: 1.6, smoke: 0.6, dirt: 0.6, chunks: 0.3 },
  bomb: { fire: 1.5, sparks: 1.4, smoke: 1.8, dirt: 1.8, chunks: 1.5 },
  wreck: { fire: 1.2, sparks: 1.1, smoke: 2, dirt: 0.5, chunks: 1, wreck: true },
} as const satisfies Record<string, ExplosionProfile>

/** Beyond this the sparks, smoke and debris are not worth drawing, metres. */
const FAR = 150
/** Beyond this the tumbling chunks (meshes) are skipped, metres. */
const CHUNK_RANGE = 80

const rand = (low: number, high: number) => low + Math.random() * (high - low)

/** Dust and dirt colour by what the blast landed on. */
const DIRT: Record<number, number> = {
  [SURFACE_ASPHALT]: 0x8d8d8d,
  [SURFACE_DIRT]: 0x7a5f3f,
  [SURFACE_SAND]: 0xd2bd88,
}

const chunkGeometry = new THREE.BoxGeometry(1, 1, 1)

/**
 * A blast of `radius` metres at `at`, in layers: a fireball, sparks and embers on ballistic arcs, a
 * ring of dust along the ground, dirt (or metal, for a wreck) thrown up, and smoke that stands
 * after the fire is gone. Everything is particles or `debris`, so there is nothing to clean up.
 */
export function explosion(world: FxWorld, debris: Debris, at: THREE.Vector3, radius: number, profile: ExplosionProfile) {
  const distance = at.distanceTo(world.camera)
  if (distance > FAR) return
  // A crowded room thins its blasts before the pools start evicting each other.
  const budget = world.smoke.alive > 2000 ? 0.5 : 1
  const count = (n: number, weight: number) => Math.max(1, Math.round(n * weight * budget))
  const scale = radius / 5
  const surface = world.surfaceAt(at.x, at.z)
  const water = surface === SURFACE_WATER
  const earth = profile.wreck ? 0x2b2b2b : water ? 0xdfeaf0 : DIRT[surface] ?? 0x5a4a30

  // Fireball: a few fat orange puffs that swell fast and float up, with a white-hot core.
  for (let i = 0; i < count(12, profile.fire); i++) {
    const heading = rand(0, Math.PI * 2)
    const out = rand(0, radius * 0.5)
    world.smoke.spawn({
      position: [at.x + Math.cos(heading) * out * 0.4, at.y + rand(0, 0.5), at.z + Math.sin(heading) * out * 0.4],
      velocity: [Math.cos(heading) * out * 1.2, rand(1.5, 5) * scale, Math.sin(heading) * out * 1.2],
      life: rand(0.5, 0.95),
      size: [radius * 0.25, radius * rand(0.7, 1.1)],
      color: i % 3 === 0 ? 0xffb347 : i % 3 === 1 ? 0xff7a1a : 0xc4400f,
      alpha: 0.9,
      gravity: -2,
      drag: 2,
    })
  }
  for (let i = 0; i < count(8, profile.fire); i++) {
    world.glow.spawn({
      position: at,
      velocity: [rand(-1, 1) * radius * 0.6, rand(0.5, 3) * scale, rand(-1, 1) * radius * 0.6],
      life: rand(0.18, 0.4),
      size: [radius * 0.45, radius * 0.9],
      color: i % 2 ? 0xfff0c0 : 0xffd27a,
      alpha: 0.9,
      drag: 3,
    })
  }

  // Sparks and embers: fast, hot, falling in arcs; some bounce off the ground and skitter.
  for (let i = 0; i < count(32, profile.sparks); i++) {
    const heading = rand(0, Math.PI * 2)
    const speed = rand(6, 22) * Math.sqrt(scale)
    const lift = rand(0.2, 1)
    world.glow.spawn({
      position: at,
      velocity: [Math.cos(heading) * speed * (1 - lift * 0.5), speed * lift, Math.sin(heading) * speed * (1 - lift * 0.5)],
      life: rand(0.4, 1.3),
      size: [rand(0.08, 0.16), 0.02],
      color: i % 4 === 0 ? 0xffffff : i % 4 === 1 ? 0xffe08a : i % 4 === 2 ? 0xffa23a : 0xff6a1a,
      alpha: 1,
      gravity: 9.8,
      drag: 0.5,
      bounce: i % 3 === 0,
    })
  }
  // A few embers that drift on the hot air instead of falling.
  for (let i = 0; i < count(6, profile.fire); i++) {
    world.glow.spawn({
      position: [at.x + rand(-1, 1), at.y + rand(0, 1), at.z + rand(-1, 1)],
      velocity: [rand(-2, 2), rand(2, 6) * scale, rand(-2, 2)],
      life: rand(1.2, 2.4),
      size: [0.12, 0.03],
      color: 0xff8a2a,
      alpha: 0.9,
      gravity: -0.6,
      drag: 1,
    })
  }

  // Dust ring: low puffs thrown out along the ground, tinted by what it is.
  for (let i = 0; i < count(12, profile.dirt); i++) {
    const heading = (i / 12) * Math.PI * 2 + rand(-0.2, 0.2)
    const speed = rand(4, 9) * Math.sqrt(scale)
    world.smoke.spawn({
      position: [at.x, world.groundAt(at.x, at.z) + 0.3, at.z],
      velocity: [Math.cos(heading) * speed, rand(0.2, 1.2), Math.sin(heading) * speed],
      life: rand(1.2, 2.2),
      size: [radius * 0.2, radius * rand(0.5, 0.8)],
      color: profile.wreck ? 0x4a4540 : water ? 0xffffff : earth,
      alpha: 0.5,
      drag: 1.8,
      gravity: -0.15,
    })
  }

  // Dirt (or wreck bits) thrown up and landing: small, hard, bouncing.
  for (let i = 0; i < count(10, profile.dirt); i++) {
    const heading = rand(0, Math.PI * 2)
    const out = rand(1, 7) * Math.sqrt(scale)
    world.smoke.spawn({
      position: at,
      velocity: [Math.cos(heading) * out, rand(5, 14) * Math.sqrt(scale), Math.sin(heading) * out],
      life: rand(1.5, 3),
      size: [rand(0.12, 0.28), 0.1],
      color: i % 3 === 0 && !profile.wreck ? 0x3d3022 : earth,
      alpha: 1,
      gravity: 9.8,
      bounce: true,
    })
  }

  // Smoke column: dark, slow, standing well after the flash has gone.
  for (let i = 0; i < count(8, profile.smoke); i++) {
    world.smoke.spawn({
      position: [at.x + rand(-1, 1) * scale, at.y + rand(0, 1.5), at.z + rand(-1, 1) * scale],
      velocity: [rand(-0.8, 0.8), rand(2, 4.5) * Math.sqrt(scale), rand(-0.8, 0.8)],
      life: rand(2.5, 4.5),
      size: [radius * 0.3, radius * rand(0.9, 1.4)],
      color: profile.wreck ? 0x151515 : i % 2 ? 0x3a3a3a : 0x555555,
      alpha: 0.45,
      gravity: -0.5,
      drag: 0.8,
    })
  }

  // Solid chunks that tumble and settle; skipped from far off, where they would be a speck.
  if (distance > CHUNK_RANGE) return
  for (let i = 0; i < count(3, profile.chunks); i++) {
    const heading = rand(0, Math.PI * 2)
    const out = rand(2, 7) * Math.sqrt(scale)
    const size = rand(0.12, 0.35)
    const material = new THREE.MeshLambertMaterial({ color: profile.wreck && i % 2 ? 0x555a60 : earth })
    const chunk = new THREE.Mesh(chunkGeometry, material)
    chunk.scale.set(size * rand(0.8, 1.6), size * rand(0.5, 1), size * rand(0.8, 1.6))
    chunk.position.copy(at)
    debris.throw(
      chunk,
      new THREE.Vector3(Math.cos(heading) * out, rand(6, 13) * Math.sqrt(scale), Math.sin(heading) * out),
      new THREE.Vector3(rand(-8, 8), rand(-8, 8), rand(-8, 8)),
      size * 0.4,
      4,
      [material],
    )
  }
}
