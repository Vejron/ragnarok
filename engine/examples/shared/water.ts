import { Color, ShaderMaterial } from '../../src'

/**
 * The game's toy-plastic water (client/src/render/water.ts) as a ShaderMaterial. The geometry's custom
 * attribute carries what the three version took as attributes: x is depth below the surface, yzw the
 * stream flow (along, across, is-stream).
 */
const FOAM_DEPTH = 0.35
const FLOW_SPEED = 1.4

const WGSL = /* wgsl */ `
fn bands(p: vec2f, t: f32) -> f32 {
  let a = sin(p.x * 0.9 + p.y * 0.35 + t * 0.9);
  let b = sin(p.x * -0.4 + p.y * 1.1 - t * 0.7);
  let c = sin((p.x + p.y) * 0.25 + t * 0.4);
  return (a + b + 0.6 * c) / 2.6;
}

fn swell(p: vec2f, t: f32) -> f32 {
  return 0.5 * sin(p.x * 0.9 + p.y * 0.35 + t * 0.9)
       + 0.35 * sin(p.x * -0.4 + p.y * 1.1 - t * 0.7)
       + 0.15 * sin((p.x + p.y) * 1.7 + t * 1.3);
}

fn caustics(p: vec2f, t: f32) -> f32 {
  let a = abs(sin(p.x * 1.3 + sin(p.y * 0.9 + t * 0.8) * 1.5));
  let b = abs(sin(p.y * 1.1 + sin(p.x * 1.2 - t * 0.6) * 1.5));
  return pow(1.0 - a * b, 6.0);
}

fn surface(s: Surface) -> vec4f {
  let time = frame.fog.w;
  let depth = s.custom.x;
  let flow = s.custom.yzw;
  let world = s.world.xz;
  let stream = flow.z > 0.5;
  var p = world * 0.7;
  if (stream) {
    p = vec2f(flow.x - time * ${FLOW_SPEED.toFixed(2)}, flow.y * 3.0) * 2.2;
  }
  let wave = bands(p, select(time, 0.0, stream));
  let band = smoothstep(0.35, 0.5, wave) * 0.5 + smoothstep(0.6, 0.7, wave) * 0.5;

  let deep = smoothstep(0.0, 2.2, depth);
  var color = mix(params.shallow, params.deep, deep);

  let clear = 1.0 - smoothstep(0.0, 1.4, depth);
  let bedP = select(world * 0.9, p * 0.8, stream);
  let web = caustics(bedP, select(time * 0.8, time, stream));
  color = mix(color, params.bed, clear * 0.45);
  color += params.foam * web * clear * 0.35;
  color = mix(color, params.foam, band * 0.12);

  let e = 0.15;
  let q = select(world * 0.8, p, stream);
  let t = select(time, 0.0, stream);
  let h = swell(q, t);
  let grad = vec2f(swell(q + vec2f(e, 0.0), t) - h, swell(q + vec2f(0.0, e), t) - h) / e;
  var n = normalize(vec3f(-grad.x * 0.12, 1.0, -grad.y * 0.12));
  if (!s.front) {
    n.y = -n.y;
  }
  let view = normalize(frame.cameraPos.xyz - s.world);

  let fresnel = pow(1.0 - clamp(abs(dot(n, view)), 0.0, 1.0), 3.0);
  let lit = floor(clamp(dot(n, frame.sunDir.xyz) * 0.5 + 0.5, 0.0, 1.0) * 3.0) / 3.0;
  color *= 0.9 + 0.2 * lit;
  color = mix(color, params.sky, fresnel * 0.55);

  let glint = pow(max(dot(reflect(-frame.sunDir.xyz, n), view), 0.0), 90.0);
  let spark = smoothstep(0.35, 0.6, glint);
  color += params.sun * spark * 0.8;

  let shore = ${FOAM_DEPTH.toFixed(2)} + 0.08 * sin(time * 1.3 + world.x * 0.6 + world.y * 0.45);
  let foam = 1.0 - smoothstep(shore * 0.55, shore, depth);
  let lace = smoothstep(0.1, 0.55, bands(world * 2.1, time * 1.6) * 0.5 + 0.5);
  color = mix(color, params.foam, foam * mix(0.55, 1.0, lace));

  var alpha = smoothstep(0.0, 0.1, depth) * mix(0.62, 0.9, deep);
  alpha = clamp(alpha - clear * 0.12 + fresnel * 0.15 + spark * 0.4, 0.0, 1.0);
  return vec4f(color, alpha);
}
`

export function waterMaterial() {
  return new ShaderMaterial({
    wgsl: WGSL,
    uniforms: {
      shallow: Color.hex(0x7fd8f5),
      deep: Color.hex(0x2f8fd8),
      foam: Color.hex(0xf4fcff),
      bed: Color.hex(0xd9cf9a),
      sky: Color.hex(0xffffff),
      sun: Color.hex(0xffffff),
    },
    blend: 'alpha',
    depthWrite: false,
    side: 'double',
    depthBias: 12,
    depthBiasSlope: 2,
  })
}
