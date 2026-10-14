import * as THREE from 'three'
import { ShaderPass } from 'three/examples/jsm/postprocessing/ShaderPass.js'

/** The outline colour: a dark ink rather than black, so it sits in the palette. */
const INK = 0x2a2521

const SHADER = {
  uniforms: {
    tDiffuse: { value: null as THREE.Texture | null },
    tDepth: { value: null as THREE.Texture | null },
    resolution: { value: new THREE.Vector2(1, 1) },
    cameraNear: { value: 0.25 },
    cameraFar: { value: 2000 },
    ink: { value: new THREE.Color(INK) },
    /** How far depth may jump between neighbours, as a share of the nearer one, before it counts as an edge. */
    threshold: { value: 0.05 },
    /** How far apart the samples are, in pixels: the line's width. */
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
    uniform vec3 ink;
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
      gl_FragColor = vec4(mix(color.rgb, ink, step(threshold, jump)), color.a);
    }
  `,
}

/**
 * Ink outlines: a line wherever depth jumps, so every silhouette (a tree against the sky, a car against
 * the grass) gets a dark edge. It reads the depth of the frame it is given, so it must come straight
 * after the scene is rendered, before anything that writes to the buffers.
 */
export class InkPass extends ShaderPass {
  constructor(private readonly camera: THREE.PerspectiveCamera) {
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
    super.render(renderer, writeBuffer, readBuffer, deltaTime, maskActive)
  }
}
