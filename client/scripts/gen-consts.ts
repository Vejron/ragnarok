// Generates `src/sim/consts.generated.ts` from `crates/shared/src/consts.rs`, which is the one place
// those constants are written down. Run `npm run gen:consts` after editing the Rust; `--check` fails
// instead of writing, and the consts spec does the same, so a stale file cannot be committed.
import { readFileSync, writeFileSync } from 'node:fs'
import { dirname, resolve } from 'node:path'
import { fileURLToPath } from 'node:url'

const here = dirname(fileURLToPath(import.meta.url))
export const RUST_SOURCE = resolve(here, '../../crates/shared/src/consts.rs')
export const GENERATED = resolve(here, '../src/sim/consts.generated.ts')

const HEADER = `// GENERATED from crates/shared/src/consts.rs by scripts/gen-consts.ts. Do not edit:
// change the Rust and run \`npm run gen:consts\`. Client-only constants live in consts.ts.
`

interface Const {
  name: string
  deps: string[]
  code: string
  comment: string[]
  gap: boolean
}

const INTEGER = /^(?:u|i)(?:8|16|32|64|size)$/

/** Turns a Rust constant expression into a TypeScript one, or returns undefined if it is not plain numbers, constants and arithmetic. */
const translate = (expr: string, integer: boolean): { code: string; deps: string[] } | undefined => {
  let source = expr
    .replace(/\s+as\s+[A-Za-z0-9_]+/g, '')
    .replace(/(\d)_(?=\d)/g, '$1')
    .replace(/(\d(?:\.\d+)?(?:e-?\d+)?)(?:f32|f64|u8|u16|u32|u64|i8|i16|i32|i64|usize)\b/g, '$1')
    .replace(/\bstd::f32::consts::PI\b/g, 'Math.PI')
    .replace(/\bstd::f32::consts::TAU\b/g, '(Math.PI * 2)')
    .trim()
  const deps = [...source.matchAll(/\b[A-Z][A-Z0-9_]*\b/g)].map((m) => m[0])
  if (!/^[\w\s.+\-*/%()<>,[\]]*$/.test(source.replace(/Math\.PI/g, ''))) return undefined
  if (/[a-z]/.test(source.replace(/Math\.PI|\be-?\d|\d+e-?\d+|0x[0-9a-f]+/gi, '').replace(/\b[A-Z][A-Z0-9_]*\b/g, ''))) return undefined
  if (source.startsWith('[')) source += ' as const'
  else if (integer && source.includes('/')) source = `Math.trunc(${source})`
  return { code: source, deps }
}

const docOf = (lines: string[]): string[] => {
  if (lines.length === 0) return []
  if (lines.length === 1) return [`/** ${lines[0]} */`]
  return ['/**', ...lines.map((line) => (line ? ` * ${line}` : ' *')), ' */']
}

export const generateConsts = (rust: string): string => {
  const consts: Const[] = []
  let docs: string[] = []
  let plain: string[] = []
  let gap = false
  for (const raw of rust.split('\n')) {
    const line = raw.trim()
    if (line.startsWith('//!')) continue
    if (line.startsWith('///')) {
      docs.push(line.slice(3).replace(/^ /, ''))
      continue
    }
    if (line.startsWith('//')) {
      plain.push(line.slice(2).replace(/^ /, ''))
      continue
    }
    const match = line.match(/^pub const ([A-Z][A-Z0-9_]*):\s*(.+?)\s*=\s*(.+?);$/)
    if (match) {
      const [, name, type, expr] = match as unknown as [string, string, string, string]
      const translated = translate(expr, INTEGER.test(type))
      if (translated && !/^[(&]/.test(type)) {
        consts.push({
          name,
          deps: translated.deps,
          code: translated.code,
          comment: [...plain.map((text) => `// ${text}`), ...docOf(docs)],
          gap,
        })
      }
    }
    if (line === '') gap = true
    else if (match) gap = false
    docs = []
    plain = []
  }

  // Rust takes constants in any order; a `const` in TypeScript must be declared before it is read.
  const emitted = new Set<string>()
  const known = new Set(consts.map((c) => c.name))
  const pending = consts.filter((c) => c.deps.every((dep) => known.has(dep)))
  // A constant that reads one we could not translate is dropped, and so on down the chain.
  for (let changed = true; changed; ) {
    changed = false
    const names = new Set(pending.map((c) => c.name))
    for (let i = pending.length - 1; i >= 0; i--) {
      if (!pending[i]!.deps.every((dep) => names.has(dep))) {
        pending.splice(i, 1)
        changed = true
      }
    }
  }
  const out: string[] = []
  while (pending.length > 0) {
    const index = pending.findIndex((c) => c.deps.every((dep) => emitted.has(dep)))
    if (index < 0) throw new Error(`constants depend on each other: ${pending.map((c) => c.name).join(', ')}`)
    const [next] = pending.splice(index, 1)
    emitted.add(next!.name)
    if (next!.gap || next!.comment.length > 0) out.push('')
    out.push(...next!.comment, `export const ${next!.name} = ${next!.code}`)
  }
  return `${HEADER}${out.join('\n').replace(/\n{3,}/g, '\n\n')}\n`
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  const next = generateConsts(readFileSync(RUST_SOURCE, 'utf8'))
  if (process.argv.includes('--check')) {
    let current = ''
    try {
      current = readFileSync(GENERATED, 'utf8')
    } catch {}
    if (current !== next) {
      console.error('consts.generated.ts is stale: run `npm run gen:consts`')
      process.exit(1)
    }
  } else {
    writeFileSync(GENERATED, next)
  }
}
