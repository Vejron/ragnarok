#!/usr/bin/env node
// Writes a brotli copy (`<file>.br`) next to every compressible file of the built client, for the
// server's static files to send in its place: Salvo's `StaticDir` serves a `.br` sidecar, with
// `Content-Encoding: br`, to any browser that accepts it. Only brotli is written: `StaticDir` takes
// the encodings in the order the browser lists them, and browsers list gzip first, so a `.gz` beside
// it would be sent instead of the smaller `.br`.
//
// Usage: node scripts/precompress.mjs [dir]   (default: dist, relative to the client)
import { readdir, readFile, rm, stat, writeFile } from 'node:fs/promises'
import { availableParallelism } from 'node:os'
import { extname, join, relative, resolve } from 'node:path'
import { promisify } from 'node:util'
import { brotliCompress, constants } from 'node:zlib'

const compress = promisify(brotliCompress)

/** The kinds of file worth compressing: text and the game's own binary formats, not images or sound, which are already compressed. */
const EXTENSIONS = new Set(['.js', '.mjs', '.css', '.html', '.wasm', '.json', '.msgpack', '.glb', '.svg', '.webmanifest', '.txt', '.xml', '.ico'])
/** Files smaller than this, bytes, gain too little to be worth a second file. */
const MIN_SIZE = 1024
/** A copy is kept only if it is at most this fraction of the original's size. */
const MAX_RATIO = 0.9
/** Files larger than this, bytes, are compressed at a lower quality: quality 11 runs at about 1 MB/s. */
const LARGE = 4 << 20

const root = resolve(process.argv[2] ?? join(import.meta.dirname, '..', 'dist'))

async function* walk(dir) {
  for (const entry of await readdir(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) yield* walk(path)
    else if (entry.isFile()) yield path
  }
}

const files = []
for await (const path of walk(root)) {
  // Copies from an earlier run are made again, so a changed file never keeps a stale one.
  if (path.endsWith('.br')) await rm(path)
  else if (EXTENSIONS.has(extname(path).toLowerCase())) files.push(path)
}

let before = 0
let after = 0
let written = 0
let next = 0
async function worker() {
  while (next < files.length) {
    const path = files[next++]
    const { size } = await stat(path)
    if (size < MIN_SIZE) continue
    const data = await readFile(path)
    const packed = await compress(data, {
      params: {
        [constants.BROTLI_PARAM_QUALITY]: size > LARGE || path.includes('/tiles/') ? 9 : 11,
        [constants.BROTLI_PARAM_SIZE_HINT]: size,
      },
    })
    if (packed.length > size * MAX_RATIO) continue
    await writeFile(`${path}.br`, packed)
    before += size
    after += packed.length
    written++
  }
}
const started = Date.now()
await Promise.all(Array.from({ length: Math.max(1, availableParallelism() - 1) }, worker))

const mb = (bytes) => `${(bytes / 1048576).toFixed(1)} MB`
console.log(`precompress: ${written} of ${files.length} files in ${relative(process.cwd(), root) || '.'}, ${mb(before)} -> ${mb(after)} brotli, ${((Date.now() - started) / 1000).toFixed(1)} s`)
