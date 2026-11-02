import type { HudStats } from '@/hud'
import { BACKPACK_NAME, type BackpackView } from '@/backpack'
import { WEAPON_AP, WEAPON_AUTOCANNON, WEAPON_BOMB, WEAPON_EGG, WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_ROCKET, WEAPON_SHELL, WEAPON_WRECK } from '@/net/codec'
import { colorFor, isBot, nameFor } from '@/profile'
import { seatHint } from '@/seathints'
import type { Painter, TextStyle } from './paint'

const INK = '#1d2b36'
const TEAL = 0x2ec4b6
const RED = 0xff5a5f
const ORANGE = 0xff8c00

const who = (id: number) => (id === 0 ? 'the fall' : nameFor(id))
/** Seconds as m:ss, for the time held on the scoreboard. */
const clock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
/** The line under the objective in a world with the backpack: who has it. */
export const backpackLine = (b: BackpackView) =>
  b.yours ? `You carry the ${BACKPACK_NAME}: you heal, and run twice as fast on foot` : b.carrier === null ? `The ${BACKPACK_NAME} is up for grabs: find it in the orange circle` : `${nameFor(b.carrier)} has the ${BACKPACK_NAME}`
/** A side's heading on the scoreboard. */
const sideName = (team: number) => (team === 0 ? 'People' : `Team ${team}`)
/** What the kill feed shows between killer and victim. */
const mark = (weapon: number) =>
  weapon === WEAPON_SHELL || weapon === WEAPON_AP || weapon === WEAPON_AUTOCANNON || weapon === WEAPON_ROCKET || weapon === WEAPON_GRENADE || weapon === WEAPON_WRECK || weapon === WEAPON_BOMB || weapon === WEAPON_EGG
    ? '💥'
    : weapon === WEAPON_REVOLVER ? '›‹' : '›'

/** A run of text pieces and colour dots laid out left to right. */
type Piece = { text: string; style: TextStyle } | { dot: number } | { tag: string }

const TAG: TextStyle = { size: 9, weight: 700, color: '#fff', tracking: 0.05 }

function pieceWidth(p: Painter, piece: Piece): number {
  if ('dot' in piece) return 18
  if ('tag' in piece) return p.measure(piece.tag, TAG) + 16
  return p.measure(piece.text, piece.style)
}

function drawPieces(p: Painter, pieces: Piece[], x: number, cy: number) {
  for (const piece of pieces) {
    if ('dot' in piece) p.dot(x + 7, cy, 5, piece.dot)
    else if ('tag' in piece) {
      const w = p.measure(piece.tag, TAG)
      p.box(x + 6, cy - 6, w + 10, 12, 4, 0x1d2b36)
      p.label(piece.tag, x + 11, cy, TAG)
    } else p.label(piece.text, x, cy, piece.style)
    x += pieceWidth(p, piece)
  }
}

/** The panels of text: status pills, prompts, hints, the scenario's notices, the kill feed, the scoreboard, death and the outcome. */
export function drawPanels(p: Painter, s: HudStats) {
  status(p, s)
  if (s.prompt) {
    const text = s.touch ? s.prompt.replace(/^E:/, 'USE:') : s.prompt
    const style: TextStyle = { size: 16, weight: 800, color: INK }
    const w = p.measure(text, style) + 36
    const y = p.height * 0.82 - 36
    p.pill(p.width / 2 - w / 2, y, w, 36, 0xffffff, 0.9)
    p.label(text, p.width / 2, y + 18, style, 'center')
  }
  const hint = !s.touch ? seatHint(s.seat, s) : null
  if (hint) {
    const style: TextStyle = { size: 13, weight: 700, color: INK }
    const w = p.measure(hint, style) + 24
    p.pill(p.width / 2 - w / 2, p.height - 12 - 24, w, 24, 0xffffff, 0.8)
    p.label(hint, p.width / 2, p.height - 24, style, 'center')
  }
  if (s.dead) {
    p.rect(0, 0, p.width, p.height, 0x961414, 0.35)
    title(p, 'ELIMINATED', `Respawning in ${Math.ceil(s.respawnIn)}`)
  }
  scenario(p, s)
  if (s.notices.outcome) {
    const won = s.notices.outcome.won
    p.rect(0, 0, p.width, p.height, won ? 0x147850 : 0x961414, 0.3)
    title(p, won ? 'VICTORY' : 'DEFEAT', s.notices.outcome.text)
  }
  feed(p, s)
  if (s.board.length) board(p, s)
}

function title(p: Painter, heading: string, line: string) {
  const size = Math.max(32, Math.min(72, p.width * 0.08))
  const cy = p.height / 2
  p.label(heading, p.width / 2, cy - size * 0.35, { size, weight: 800, color: '#fff', shadow: true, tracking: 0.08 }, 'center')
  p.label(line, p.width / 2, cy + size * 0.45, { size: 20, weight: 700, color: '#fff', shadow: true }, 'center')
}

/** The top-left pills: the connection, ping, players, voice, the tally and the debug line. */
function status(p: Painter, s: HudStats) {
  const pills: { text: string; color: string; weight?: number; alpha?: number }[] = []
  const state = s.status === 'CONNECTED' ? 'ONLINE' : s.status === 'CONNECTING' ? 'CONNECTING' : 'SOLO'
  pills.push({ text: state, color: s.status === 'CONNECTED' ? '#138a4b' : s.status === 'CONNECTING' ? '#b26b00' : '#b3261e' })
  if (s.rtt !== null) pills.push({ text: `${Math.round(s.rtt)} ms`, color: INK })
  pills.push({ text: `${s.players} ${s.players === 1 ? 'player' : 'players'} in view`, color: INK })
  if (s.status === 'CONNECTED' && s.voice.mic !== 'unavailable') {
    const mic = s.voice.mic === 'denied' ? 'mic blocked' : s.voice.talking ? 'talking' : 'V to talk'
    pills.push({ text: `${mic}${s.voice.peers > 0 ? ` · ${s.voice.peers} in earshot` : ''}`, color: s.voice.talking ? '#138a4b' : INK, weight: s.voice.talking ? 600 : 700 })
  }
  if (s.status === 'CONNECTED') pills.push({ text: `${s.kills} kills · ${s.deaths} deaths`, color: INK })
  pills.push({ text: `tiles ${s.tiles} · interp ${Math.round(s.interpDelayMs)} ms · fix ${s.correction.toFixed(2)} m · unacked ${s.pending}`, color: INK, weight: 500, alpha: 0.8 })
  // Wrap before the minimap.
  const limit = p.width - 12 - 184 - 12
  let x = 12
  let y = 12
  for (const pill of pills) {
    const style: TextStyle = { size: 13, weight: pill.weight ?? 700, color: pill.color }
    const w = p.measure(pill.text, style) + 20
    if (x > 12 && x + w > limit) {
      x = 12
      y += 32
    }
    p.pill(x, y, w, 24, 0xffffff, 0.8 * (pill.alpha ?? 1))
    p.label(pill.text, x + 10, y + 12, style, 'left', pill.alpha ?? 1)
    x += w + 8
  }
}

/** Under the top pills, centred: the objective, who has the backpack, and the scenario's messages. */
function scenario(p: Painter, s: HudStats) {
  const lines: { text: string; style: TextStyle; fill: number; fillAlpha: number; ring: number | null }[] = []
  if (s.notices.objective) lines.push({ text: s.notices.objective, style: { size: 15, weight: 800, color: INK }, fill: 0xffffff, fillAlpha: 0.85, ring: TEAL })
  if (s.backpack) {
    const yours = s.backpack.yours
    lines.push({ text: backpackLine(s.backpack), style: { size: 16, weight: 700, color: yours ? '#fff' : INK }, fill: yours ? ORANGE : 0xffffff, fillAlpha: yours ? 1 : 0.85, ring: ORANGE })
  }
  for (const text of s.notices.messages) lines.push({ text, style: { size: 14, weight: 700, color: INK }, fill: 0xffffff, fillAlpha: 0.85, ring: null })
  let y = 52
  for (const line of lines) {
    const w = Math.min(p.width - 32, p.measure(line.text, line.style) + 28)
    const h = line.style.size * 1.3 + 12
    const x = p.width / 2 - w / 2
    p.box(x, y, w, h, 10, line.fill, line.fillAlpha, line.ring === null ? 0 : 2, line.ring ?? 0, 1)
    p.label(line.text, p.width / 2, y + h / 2, line.style, 'center')
    y += h + 4
  }
}

/** The kill feed, on the right under the minimap, oldest first. */
function feed(p: Painter, s: HudStats) {
  const style: TextStyle = { size: 13, weight: 700, color: INK }
  let y = 206
  for (const entry of s.feed) {
    const pieces: Piece[] = []
    if (entry.killer !== 0) {
      pieces.push({ dot: colorFor(entry.killer) }, { text: who(entry.killer), style })
      if (isBot(entry.killer)) pieces.push({ tag: 'BOT' })
      pieces.push({ text: `  ${mark(entry.weapon)}  `, style: { ...style, weight: 800 } })
    }
    pieces.push({ dot: colorFor(entry.victim) }, { text: who(entry.victim), style })
    if (isBot(entry.victim)) pieces.push({ tag: 'BOT' })
    if (entry.killer === 0) pieces.push({ text: ` took ${who(0)}`, style })
    const w = pieces.reduce((sum, piece) => sum + pieceWidth(p, piece), 0) + 20
    const x = p.width - 12 - w
    const ring = entry.you === 'killer' ? TEAL : entry.you === 'victim' ? RED : null
    p.pill(x, y, w, 24, 0xffffff, 0.85, ring === null ? 0 : 2, ring ?? 0, 1)
    drawPieces(p, pieces, x + 10, y + 12)
    y += 28
  }
}

/** The scoreboard, while Tab is held: every player by side. */
function board(p: Painter, s: HudStats) {
  const head: TextStyle = { size: 12, weight: 700, color: 'rgba(29, 43, 54, 0.6)' }
  const sideStyle: TextStyle = { size: 13, weight: 700, color: INK }
  const cell = (you: boolean): TextStyle => ({ size: 16, weight: 700, color: you ? '#138a4b' : INK })
  const columns = s.backpack ? ['Kills', 'Deaths', 'Backpack'] : ['Kills', 'Deaths']
  const nameOf = (id: number, you: boolean) => (you ? `${nameFor(id)} (you)` : nameFor(id))
  let nameWidth = p.measure('Player', head)
  for (const side of s.board) for (const row of side.rows) nameWidth = Math.max(nameWidth, 18 + p.measure(nameOf(row.id, row.you), cell(row.you)) + (isBot(row.id) ? p.measure('BOT', TAG) + 16 : 0))
  const colWidth = 76
  const width = Math.max(320, 16 * 2 + nameWidth + 16 + columns.length * colWidth)
  const rowH = 28
  const rows = s.board.reduce((n, side) => n + side.rows.length + (s.board.length > 1 ? 1 : 0), 1)
  const height = 20 + rows * rowH + (s.board.length > 1 ? s.board.length * 6 : 0)
  const x = p.width / 2 - width / 2
  let y = p.height / 2 - height / 2
  p.box(x, y, width, height, 14, 0xffffff, 0.92)
  const right = (k: number) => x + width - 16 - (columns.length - 1 - k) * colWidth
  y += 10
  p.label('Player', x + 16, y + rowH / 2, head)
  columns.forEach((column, k) => p.label(column, right(k), y + rowH / 2, head, 'right'))
  y += rowH
  for (const side of s.board) {
    if (s.board.length > 1) {
      y += 6
      p.label(sideName(side.team), x + 16, y + rowH / 2, sideStyle)
      const values = [String(side.kills), String(side.deaths), clock(side.held)]
      columns.forEach((_, k) => p.label(values[k]!, right(k), y + rowH / 2, sideStyle, 'right'))
      p.rect(x + 16, y + rowH - 1, width - 32, 1, 0x1d2b36, 0.15)
      y += rowH
    }
    for (const row of side.rows) {
      const style = cell(row.you)
      const pieces: Piece[] = [{ dot: colorFor(row.id) }, { text: nameOf(row.id, row.you), style }]
      if (isBot(row.id)) pieces.push({ tag: 'BOT' })
      drawPieces(p, pieces, x + 16, y + rowH / 2)
      const values = [String(row.kills), String(row.deaths), clock(row.held)]
      columns.forEach((_, k) => p.label(values[k]!, right(k), y + rowH / 2, style, 'right'))
      y += rowH
    }
  }
}
