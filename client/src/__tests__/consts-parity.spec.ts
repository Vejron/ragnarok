import { readdirSync, readFileSync } from 'node:fs'
import { resolve } from 'node:path'
import { describe, expect, it } from 'vitest'

// Every numeric `pub const` in `crates/shared/src/*.rs` that the client also exports under the same
// name must have the same value. The Rust side is read from source, so a new constant on either side
// is covered without listing it anywhere. A constant only one side has is not checked, and neither
// is one whose value is not a plain number, array or arithmetic of those.
const SHARED = resolve(process.cwd(), '../crates/shared/src')

type Value = number | number[]

/** Evaluates a Rust constant expression made of numbers, known constants, arithmetic and casts. */
const evaluate = (expr: string, known: Map<string, Value>): Value | undefined => {
  let source = expr
    .replace(/\bas\s+[A-Za-z0-9_]+/g, '')
    .replace(/(\d)_(?=\d)/g, '$1')
    .replace(/(\d(?:\.\d+)?(?:e-?\d+)?)(?:f32|f64|u8|u16|u32|u64|i8|i16|i32|i64|usize)\b/g, '$1')
    .replace(/\bstd::f32::consts::PI\b|\bstd::f32::consts::TAU\b/g, (m) => String(m.endsWith('PI') ? Math.PI : Math.PI * 2))
    .replace(/\b(?:f32|f64)::(MAX|MIN|INFINITY)\b/g, 'NaN')
    .trim()
  if (source.startsWith('[') && source.endsWith(']')) {
    const parts = source.slice(1, -1).split(',').map((part) => part.trim()).filter(Boolean)
    const values = parts.map((part) => evaluate(part, known))
    return values.every((v) => typeof v === 'number') ? (values as number[]) : undefined
  }
  let unknown = false
  source = source.replace(/\b[A-Z][A-Z0-9_]*\b/g, (name) => {
    const value = known.get(name)
    if (typeof value !== 'number') {
      unknown = true
      return 'NaN'
    }
    return `(${value})`
  })
  if (unknown || !/^[\d\s.eE+\-*/%()<>]+$/.test(source) || source.includes('NaN')) return undefined
  try {
    const value = new Function(`return (${source})`)() as number
    return Number.isFinite(value) ? value : undefined
  } catch {
    return undefined
  }
}

const readRustConsts = (): Map<string, Value> => {
  const found = new Map<string, Value>()
  const files = readdirSync(SHARED).filter((file) => file.endsWith('.rs')).sort()
  const declarations: [string, string][] = []
  for (const file of files) {
    const text = readFileSync(resolve(SHARED, file), 'utf8').replace(/\/\/.*$/gm, '')
    for (const match of text.matchAll(/^\s*pub const ([A-Z][A-Z0-9_]*):\s*([^=]+?)\s*=\s*([^;]+);/gm)) {
      declarations.push([match[1], match[3].replace(/\s+/g, ' ')])
    }
  }
  // Constants refer to earlier and later ones alike, so settle in passes.
  for (let pass = 0; pass < 6; pass++) {
    for (const [name, expr] of declarations) {
      if (found.has(name)) continue
      const value = evaluate(expr, found)
      if (value !== undefined) found.set(name, value)
    }
  }
  return found
}

const mirrors = import.meta.glob(['../sim/*.ts', '../net/codec.ts'], { eager: true }) as Record<string, Record<string, unknown>>

const typescriptConsts = (): Map<string, Value> => {
  const found = new Map<string, Value>()
  for (const module of Object.values(mirrors)) {
    for (const [name, value] of Object.entries(module)) {
      if (!/^[A-Z][A-Z0-9_]*$/.test(name)) continue
      if (typeof value === 'number') found.set(name, value)
      else if (Array.isArray(value) && value.every((v) => typeof v === 'number')) found.set(name, [...value] as number[])
    }
  }
  return found
}

const close = (a: number, b: number) => Math.abs(a - b) <= 1e-5 * Math.max(1, Math.abs(a), Math.abs(b))

describe('shared constants match the server', () => {
  const rust = readRustConsts()
  const client = typescriptConsts()
  const shared = [...client.keys()].filter((name) => rust.has(name)).sort()

  it('compares a meaningful number of constants', () => {
    // Guards the scan itself: if the regexes stop matching, nothing would be compared.
    expect(shared.length).toBeGreaterThan(200)
  })

  it('has the same value on both sides for every constant they share', () => {
    const drifted: string[] = []
    for (const name of shared) {
      const a = rust.get(name)!
      const b = client.get(name)!
      const same =
        typeof a === 'number' && typeof b === 'number'
          ? close(a, b)
          : Array.isArray(a) && Array.isArray(b) && a.length === b.length && a.every((v, i) => close(v, b[i]))
      if (!same) drifted.push(`${name}: rust ${JSON.stringify(a)} != client ${JSON.stringify(b)}`)
    }
    expect(drifted).toEqual([])
  })
})
