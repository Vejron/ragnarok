import * as THREE from 'three'
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js'


const SHADER = {
  uniforms: {
    tDiffuse: { value: null as THREE.Texture | null },
    tDepth: { value: null as THREE.Texture | null },
    resolution: { value: new THREE.Vector2(1, 1) },
    cameraNear: { value: 0.25 },
    cameraFar: { value: 2000 },
    /** What the line keeps of the colour under it: 0.3 leaves a third of its brightness at the darkest. */
    darken: { value: 0.3 },
    /** How much of the line shows, 0 for none and 1 for the full darkening. */
    strength: { value: 0.6 },
    /** How far depth may jump between neighbours, as a share of the nearer one, before it counts as an edge. */
    threshold: { value: 0.05 },
    /** How far apart the samples are, in buffer pixels: the line's width, set from the pixel ratio. */
    width: { value: 1.5 },
  },
  vertexShader: /* glsl */ `
    varying vec2 vUv;
    void main() {
      vUv = uv;
      gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
    }
  `,
  fragmentShader: /* glsl */ `
    uniform sampler2D tDiffuse;
    uniform sampler2D tDepth;
    uniform vec2 resolution;
    uniform float cameraNear;
    uniform float cameraFar;
    uniform float darken;
    uniform float strength;
    uniform float threshold;
    uniform float width;
    varying vec2 vUv;

    // View-space distance of the depth-buffer value at uv.
    float viewDepth(vec2 uv) {
      float z = texture2D(tDepth, uv).x * 2.0 - 1.0;
      return 2.0 * cameraNear * cameraFar / (cameraFar + cameraNear - z * (cameraFar - cameraNear));
    }

    void main() {
      vec2 texel = width / resolution;
      float c = viewDepth(vUv);
      float l = viewDepth(vUv + vec2(-texel.x, 0.0));
      float r = viewDepth(vUv + vec2(texel.x, 0.0));
      float d = viewDepth(vUv + vec2(0.0, -texel.y));
      float u = viewDepth(vUv + vec2(0.0, texel.y));
      // A silhouette or occluding edge bends the depth sharply across the pixel. A smooth surface, even a
      // long slope of ground seen at a low angle, only changes its rate, so it stays clear of the ink.
      float jump = max(abs(l + r - 2.0 * c), abs(d + u - 2.0 * c)) / c;
      vec4 color = texture2D(tDiffuse, vUv);
      // A short ramp instead of a hard step: the edge's stair steps fade into the picture.
      float edge = smoothstep(threshold, threshold * 1.6, jump);
      // The line darkens what it is drawn over rather than painting a fixed colour, so the hue carries through.
      gl_FragColor = vec4(mix(color.rgb, color.rgb * darken, edge * strength), color.a);
    }
  `,
}

/** The line's width in CSS pixels, so it looks the same whatever the screen's pixel ratio. */
const LINE_WIDTH = 1.2

/**
 * Ink outlines: a line wherever depth jumps, so every silhouette (a tree against the sky, a car against
 * the grass) gets a dark edge. It reads the depth of the frame it is given, so it must come straight
 * after the scene is rendered, before anything that writes to the buffers.
 */
export class InkPass extends ShaderPass {
  /** `pixelRatio` is the ratio the composer renders at: buffer pixels per CSS pixel. */
  constructor(private readonly camera: THREE.PerspectiveCamera, private readonly pixelRatio: () => number) {
    super(SHADER)
  }

  override setSize(width: number, height: number) {
    super.setSize(width, height)
    this.uniforms.resolution!.value.set(width, height)
  }

  override render(renderer: THREE.WebGLRenderer, writeBuffer: THREE.WebGLRenderTarget, readBuffer: THREE.WebGLRenderTarget, deltaTime: number, maskActive: boolean) {
    this.uniforms.tDepth!.value = readBuffer.depthTexture
    this.uniforms.cameraNear!.value = this.camera.near
    this.uniforms.cameraFar!.value = this.camera.far
    this.uniforms.width!.value = LINE_WIDTH * this.pixelRatio()
    super.render(renderer, writeBuffer, readBuffer, deltaTime, maskActive)
  }
}
