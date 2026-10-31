# @ragnarok/engine

A small WebGPU-only renderer. It does what a stylised outdoor game needs: toon-lit meshes, sun shadows, fog, skinned glTF models, custom shader materials, GPU particles, short-lived shot effects and ink outlines. It does not try to be general. There is no WebGL fallback, no PBR, no post-processing stack and no scene loader beyond binary glTF.

It has no runtime dependencies and imports nothing from the game.

```ts
import { Renderer, Scene, PerspectiveCamera, Mesh, Material, boxGeometry, Ramp } from '@ragnarok/engine'

const renderer = await Renderer.create(canvas)
renderer.setSize(innerWidth, innerHeight, devicePixelRatio)
const scene = new Scene()
scene.sun.position.set(70, 80, 55)
scene.sun.shadow.enabled = true
const camera = new PerspectiveCamera(60, innerWidth / innerHeight, 0.25, 2000)
camera.position.set(0, 5, 10)
camera.lookAt(0, 0, 0)
const box = new Mesh(boxGeometry(), new Material({ color: 0xc0603a, ramp: new Ramp(new Uint8Array([96, 150, 205, 255])) }))
box.castShadow = true
scene.add(box)
renderer.render(scene, camera)
```

## Why it is cheaper than three

On the benchmark scene (1,000 meshes, each with its own material, plus 2,000 instanced trees, with shadows), on the same machine:

| Renderer | Render CPU per frame | Draw calls |
|---|---|---|
| This engine | 0.4 ms | 9 |
| three `WebGLRenderer` | 2.0 ms | 1,861 |
| three `WebGPURenderer` | 3.5 ms | 1,862 |

The difference comes from how GPU state is organised, not from per-call speed:

- **One object buffer.** Every drawn object has a record holding its model matrix, tint, material index and flags. Records persist across frames and are rewritten only when the object moves or changes. Each frame uploads a *draw list* of record indices, 4 bytes per instance, and shaders read `objects[drawList[instance_index]]`.
- **Materials are data.** Material parameters sit in a storage buffer indexed by the record. Only render state (blend, cull side, depth test and write, depth bias) and the shader pick a pipeline, so a thousand materials draw with a handful of pipelines.
- **One geometry arena.** All vertices share one buffer and one layout, and all indices share another. A draw is offsets into them.
- **Automatic instancing.** Meshes that share geometry and pipeline become one `drawIndexed` call, whatever their materials, tints or skeletons.
- **Shared GPU work for effects.** Particles and flashes keep their state on the GPU and draw indirectly.

## Concepts

- **Scene graph:** `Node`, `Mesh`, `InstancedMesh`, `SkinnedMesh`, `PerspectiveCamera` and `Scene`.
  - The sun, hemisphere light, fog, ink and tone mapping are settings on `Scene`, not nodes.
  - `position`, `quaternion` and `scale` mark a node dirty when written, so only moved subtrees are recomputed.
- **Geometry:** `Geometry` holds CPU arrays: positions, normals, uvs, linear colours, a free `custom` vec4, skin joints and weights, and indices.
  - It is uploaded on first draw.
  - `needsUpdate()` re-sends everything; `needsUpdateRange(first, count)` re-sends only those vertices.
- **Materials:** `Material` is toon-lit (three's `MeshToonMaterial` maths with a `Ramp`) or unlit.
  - It supports `map`, an object-space `normalMap`, `emissive`, vertex colours, alpha or additive blending, depth bias, double-siding and fog.
  - `ShaderMaterial` takes a WGSL `surface()` function plus named uniforms and textures.
- **Textures:** `Texture` takes a canvas, an image or raw RGBA pixels, in sRGB or linear, and generates its own mipmaps.
- **Models:** `parseGlb` / `loadGlb` read binary glTF: meshes, materials (base colour), skins and animations.
- **Animation:** `Animator` blends clips with hand-set weights and times, or runs them on its own clock.
- **Effects:** an `Effect` is a node drawn by its own GPU code, after all meshes, with the scene's depth readable.
  - `ParticleSystem` simulates particles entirely on the GPU.
    - `spawn(spec)` creates one particle.
    - `emit(spec)` creates a burst, randomised on the GPU.
    - Particles bounce on a height field set with `setGround`, and have soft edges and fog.
  - `FlashEffects` holds short-lived tracer streaks and ground rings, each one evaluated in the vertex shader from its spawn record.
- **Stats:** `renderer.stats` reports draw calls, triangles, instances, record churn, pipelines, CPU time and per-pass GPU time (when the device supports `timestamp-query`).

## Using it from a game

The engine has its own small math. `Vec3`, `Quat` and `Euler` are what a node exposes: writing them tells the
node it moved. `Vector3`, `Quaternion` and `Matrix4` are plain values to calculate with, with no such cost, and
`Color` and `mat4` serve both. Anything with `x`, `y`, `z` is taken where a vector is expected, so the two mix.

- **Rotation** is a quaternion and an Euler (`node.rotation`, with an order), kept in step.
- **Hierarchy:** `attach` re-parents without moving, `localToWorld` / `worldToLocal`, `getWorldPosition`
  / `getWorldQuaternion` (bringing stale ancestors up to date first), `updateWorldMatrix(parents, children)`.
- **Geometry:** `clone`, `translate`, `scale`, `rotateX/Y/Z`, `Geometry.merge`, `toNonIndexed`; shapes
  `boxGeometry`, `sphereGeometry`, `planeGeometry`, `quadGeometry`, `ringGeometry`, `icosphereGeometry`,
  `cylinderGeometry`.
- **Overlay:** a mesh with `overlay = true` is drawn after everything including effects, ignoring depth
  (reticles, markers); use an alpha-blended material.
- **Camera:** `lookAt`, `getWorldDirection`, `project(point)` for a point's place on screen.

## Frame

1. **Effect prepare:** compute passes such as particle emission and simulation.
2. **Shadow pass:** depth only, from the sun, of shadow casters inside the sun's frustum.
3. **Main pass:** 4× MSAA into `rgba16float`, with reversed depth.
   - Opaque draws are sorted by pipeline, then textures, then geometry, and merged into instanced batches.
   - Transparent draws follow, sorted far to near.
4. **Effects pass:** the same targets, with depth bound read-only so effects can sample it.
5. **Overlay pass:** meshes marked `overlay`, over the effects, with the depth ignored; it resolves the MSAA image.
6. **Output pass:** ink outlines from depth, then exposure, tone mapping (none or Neutral) and the sRGB encode to the canvas.

## Working on it

```sh
npm install
npm run dev         # examples at http://localhost:5180/
npm test            # unit tests (math, allocator, glTF on the game's models, animation)
npm run type-check
node scripts/inspect.mjs "http://localhost:5180/static/?still" 6000 shot.png   # headless Chrome on the real GPU: errors, stats, screenshot
```

Examples:
- **`static/`:** the benchmark scene.
- **`materials/`:** textures, a normal map, the game's water as a `ShaderMaterial`, blending and a decal.
- **`particles/`:** explosions, smoke and gunfire; add `?stress=100000` to keep that many particles alive.
- **`models/`:** 64 animated players and the vehicles.
