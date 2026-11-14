// Draws the game's frame once for each of two eyes into one offscreen texture, as a headset's session would have it
// drawn, and reports GPU validation errors and how far the two halves differ (docs/VR.md, "Picking it up").
// It is an expression for the dev page: node client/scripts/cdp.mjs eval "$(cat client/scripts/vr-eyes.js)"
(async () => {
  const g = window.__game
  const scene = g.scene
  const renderer = scene.renderer
  const device = renderer.device
  const [W, H] = [640, 720]
  const texture = device.createTexture({ size: [2 * W, H], format: renderer.format, usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC })
  const view = texture.createView()
  // A symmetric lens of the eye's shape, as the engine itself would make it.
  const camera = scene.camera
  const was = camera.aspect
  camera.aspect = W / H
  camera.lens = null
  camera.updateMatrices()
  const lens = Float32Array.from(camera.projection)
  camera.aspect = was
  // Eyes a metre apart, so that the difference between them is plain to see.
  const eye = (x, index) => ({ position: [x, 0, 0], orientation: [0, 0, 0, 1], lens, fov: camera.fov, aspect: W / H, target: { view, x: index * W, y: 0, again: index > 0 }, width: W, height: H })
  const errors = []
  device.pushErrorScope('validation')
  scene.see([eye(0, 0), eye(0, 1)])
  scene.render(); scene.render(); scene.render()
  const error = await device.popErrorScope()
  if (error) errors.push(error.message)
  const sized = [renderer.width, renderer.height]
  // Read it back.
  const bytesPerRow = 2 * W * 4
  const buffer = device.createBuffer({ size: bytesPerRow * H, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ })
  const encoder = device.createCommandEncoder()
  encoder.copyTextureToBuffer({ texture }, { buffer, bytesPerRow }, [2 * W, H])
  device.queue.submit([encoder.finish()])
  await buffer.mapAsync(GPUMapMode.READ)
  const data = new Uint8Array(buffer.getMappedRange().slice(0))
  buffer.unmap()
  scene.see(null)
  const bgra = renderer.format.startsWith('bgra')
  const canvas = document.createElement('canvas')
  canvas.width = 2 * W
  canvas.height = H
  canvas.id = 'eyes2'
  canvas.style.cssText = 'position:fixed;left:0;top:0;z-index:99999;width:1280px;height:720px;opacity:1 !important'
  const image = new ImageData(2 * W, H)
  let [left, right, differ] = [0, 0, 0]
  for (let i = 0; i < data.length; i += 4) {
    image.data[i] = data[i + (bgra ? 2 : 0)]
    image.data[i + 1] = data[i + 1]
    image.data[i + 2] = data[i + (bgra ? 0 : 2)]
    image.data[i + 3] = 255
  }
  for (let y = 0; y < H; y += 4) for (let x = 0; x < W; x += 4) {
    const a = (y * 2 * W + x) * 4, b = (y * 2 * W + x + W) * 4
    left += data[a] + data[a + 1] + data[a + 2]
    right += data[b] + data[b + 1] + data[b + 2]
    if (Math.abs(data[a] - data[b]) + Math.abs(data[a + 1] - data[b + 1]) + Math.abs(data[a + 2] - data[b + 2]) > 6) differ++
  }
  canvas.getContext('2d').putImageData(image, 0, 0)
  void canvas
  const samples = (W / 4) * (H / 4)
  return { format: renderer.format, sized, errors, leftMean: left / samples / 3, rightMean: right / samples / 3, differingShare: differ / samples, xrCompatible: renderer.xrCompatible, backSize: [renderer.width, renderer.height] }
})()
