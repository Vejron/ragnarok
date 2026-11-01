import { Group, Mesh, boxGeometry, cylinderGeometry, type Geometry, type Material } from '@ragnarok/engine'
import { toonMaterial } from './toon'

/** Cross-country skis and poles, drawn on a soldier wearing them. */

const SKI_COLOR = 0xc8342b
const BINDING_COLOR = 0x20242b
const POLE_COLOR = 0x2c3038

/** Ski length, how much of it lies ahead of the foot, and its width, metres. */
const SKI_LENGTH = 1.9
const SKI_AHEAD = 1.0
const SKI_WIDTH = 0.055
/** Depth bias of a ski, over any ground drawn close under it. */
const SKI_BIAS = 40
/** How far under the ankle bone the ski lies, clear of the trails drawn on the snow, and how far along the foot it clips in, metres. */
export const SOLE_DROP = 0.045
const CLIP_AHEAD = 0.06
/** Pole length, and how much of it stands above the grip, metres. */
const POLE_LENGTH = 1.42
const POLE_ABOVE = 0.08
/** From the grip to the tip, metres. */
export const POLE_REACH = POLE_LENGTH - POLE_ABOVE
const BASKET_RADIUS = 0.055

let parts: { ski: Geometry; tip: Geometry; binding: Geometry; pole: Geometry; basket: Geometry; red: Material; dark: Material; strap: Material } | null = null

function shared() {
  return (parts ??= {
    ski: boxGeometry(SKI_WIDTH, 0.016, SKI_LENGTH),
    tip: boxGeometry(SKI_WIDTH * 0.8, 0.014, 0.16),
    binding: boxGeometry(SKI_WIDTH * 1.1, 0.03, 0.14),
    pole: cylinderGeometry(0.009, 0.006, POLE_LENGTH, 5),
    basket: cylinderGeometry(BASKET_RADIUS, BASKET_RADIUS, 0.012, 6),
    red: toonMaterial({ color: SKI_COLOR, depthBias: SKI_BIAS, depthBiasSlope: 2 }),
    dark: toonMaterial({ color: BINDING_COLOR, depthBias: SKI_BIAS, depthBiasSlope: 2 }),
    strap: toonMaterial({ color: POLE_COLOR }),
  })
}

/** One ski, the boot's place on it at the origin, pointing +Z with its top up. */
export function buildSki(): Group {
  const { ski, tip, binding, red, dark } = shared()
  const group = new Group()
  const inner = new Group()
  inner.position.set(0, -SOLE_DROP, CLIP_AHEAD)
  group.add(inner)
  const blade = new Mesh(ski, red)
  blade.position.z = SKI_AHEAD - SKI_LENGTH / 2
  inner.add(blade)
  const toe = new Mesh(tip, red)
  toe.position.set(0, 0.03, SKI_AHEAD + 0.05)
  toe.rotation.x = -0.5
  inner.add(toe)
  const clip = new Mesh(binding, dark)
  clip.position.y = 0.02
  inner.add(clip)
  return group
}

/** One pole, the grip at the origin, its tip towards -Y. */
export function buildPole(): Group {
  const { pole, basket, strap, dark } = shared()
  const group = new Group()
  const shaft = new Mesh(pole, strap)
  shaft.position.y = POLE_ABOVE - POLE_LENGTH / 2
  group.add(shaft)
  const ring = new Mesh(basket, dark)
  ring.position.y = POLE_ABOVE - POLE_LENGTH + 0.1
  group.add(ring)
  return group
}
