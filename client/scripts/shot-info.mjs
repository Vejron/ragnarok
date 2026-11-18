// Prints what an F2 screenshot says about where it was taken (its `ragnarok` iTXt chunk, `src/screenshot.ts`).
//   node client/scripts/shot-info.mjs <shot.png>
import { readFileSync } from 'node:fs'

const png = readFileSync(process.argv[2])
let found = null
for (let at = 8; at + 12 <= png.length; ) {
  const length = png.readUInt32BE(at)
  const type = png.toString('latin1', at + 4, at + 8)
  if (type === 'iTXt') {
    const data = png.subarray(at + 8, at + 8 + length)
    const end = data.indexOf(0)
    if (data.toString('latin1', 0, end) === 'ragnarok') {
      let i = end + 3
      i = data.indexOf(0, i) + 1
      i = data.indexOf(0, i) + 1
      found = JSON.parse(data.toString('utf8', i))
      break
    }
  }
  if (type === 'IEND') break
  at += 12 + length
}
if (!found) {
  console.error('no ragnarok chunk in', process.argv[2])
  process.exit(1)
}
console.log(JSON.stringify(found, null, 1))
