import type { HudStats } from '@/hud'
import { BACKPACK_NAME, type BackpackView } from '@/backpack'
import { WEAPON_AP, WEAPON_AUTOCANNON, WEAPON_BALL, WEAPON_BOMB, WEAPON_DROWN, WEAPON_EGG, WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_ROCKET, WEAPON_SHELL, WEAPON_WRECK } from '@/net/codec'
import { colorFor, isBot, isZombie, nameFor } from '@/profile'
import { seatHint } from '@/seathints'
import { miniBottom, miniSize } from './map'
import { GREEN_CSS, INK_CSS as INK, MARGIN, RED, TEAL, TEXT, type Painter, type TextStyle } from './paint'

const ORANGE = 0xff8c00
/** How long a seat's keys, the objective and the backpack's news are shown in full before they shrink or fade, ms. */
const FRESH_MS = 6000
const OBJECTIVE_MS = 8000
/** A seat's keys, once they have been read: faded, still there to look up. */
const HINT_FADED = 0.45
/** The widest a seat's keys run before they wrap, layout pixels. */
const HINT_WIDTH = 640
/** Room kept clear for the bottom-right gauge column when a seat's keys wrap. */
const CORNER_WIDTH = 176
/** What the touch buttons are called, for prompts that name a key. */
const TOUCH_KEYS: Record<string, string> = { E: 'USE', G: 'CHUTE' }

const who = (id: number) => (id === 0 ? 'the fall' : nameFor(id))
/** Seconds as m:ss, for the time held on the scoreboard. */
const clock = (seconds: number) => `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
/** The line under the objective in a world with the backpack: who has it. */
export const backpackLine = (b: BackpackView) =>
  b.yours ? `You carry the ${BACKPACK_NAME}: you heal, and run twice as fast on foot` : b.carrier === null ? `The ${BACKPACK_NAME} is up for grabs: find it in the orange circle` : `${nameFor(b.carrier)} has the ${BACKPACK_NAME}`
/** The short form of `backpackLine`, once the news is a few seconds old. */
const backpackShort = (b: BackpackView) => (b.yours ? `You carry the ${BACKPACK_NAME}` : b.carrier === null ? `${BACKPACK_NAME}: up for grabs` : `${nameFor(b.carrier)} has the ${BACKPACK_NAME}`)
/** A prompt with its key renamed for the touch buttons ("E: take" → "USE: take"). */
export const touchPrompt = (prompt: string) => prompt.replace(/^([A-Z]):/, (whole, key: string) => (TOUCH_KEYS[key] ? `${TOUCH_KEYS[key]}:` : whole))
/** A side's heading on the scoreboard. */
const sideName = (team: number) => (team === 0 ? 'People' : `Team ${team}`)
/** What the kill feed shows between killer and victim. */
const mark = (weapon: number) =>
  weapon === WEAPON_SHELL || weapon === WEAPON_AP || weapon === WEAPON_AUTOCANNON || weapon === WEAPON_ROCKET || weapon === WEAPON_GRENADE || weapon === WEAPON_WRECK || weapon === WEAPON_BOMB || weapon === WEAPON_EGG || weapon === WEAPON_BALL
    ? '💥'
    : weapon === WEAPON_REVOLVER ? '›‹' : '›'

/** A run of text pieces and colour dots laid out left to right. */
type Piece = { text: string; style: TextStyle } | { dot: number } | { tag: string }

const TAG: TextStyle = { size: TEXT.xs, weight: 700, color: '#fff', tracking: 0.05 }

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
      p.box(x + 6, cy - 7, w + 10, 14, 4, 0x1d2b36)
      p.label(piece.tag, x + 11, cy, TAG)
    } else p.label(piece.text, x, cy, piece.style)
    x += pieceWidth(p, piece)
  }
}

/** `text` broken at spaces into lines no wider than `width`, as `measure` has it. */
export function wrap(measure: (text: string) => number, text: string, width: number): string[] {
  const lines: string[] = []
  let line = ''
  for (const word of text.split(' ')) {
    const next = line ? `${line} ${word}` : word
    if (line && measure(next) > width) {
      lines.push(line)
      line = word
    } else line = next
  }
  if (line) lines.push(line)
  return lines
}

/** How strongly a seat's keys are shown `since` ms after they were last new: in full, then faded. */
export const hintAlpha = (since: number) => (since < FRESH_MS ? 1 : since < FRESH_MS + 600 ? 1 - ((since - FRESH_MS) / 600) * (1 - HINT_FADED) : HINT_FADED)

/** Lines centred at `cx`, `top` down, in a box: returns the box's height. */
function block(p: Painter, lines: string[], cx: number, top: number, style: TextStyle, fill: number, fillAlpha: number, ring: number | null, alpha = 1): number {
  const lineH = Math.ceil(style.size * 1.3)
  const w = Math.max(...lines.map((line) => p.measure(line, style))) + 28
  const h = lines.length * lineH + 12
  p.box(cx - w / 2, top, w, h, 10, fill, fillAlpha * alpha, ring === null ? 0 : 2, ring ?? 0, alpha)
  lines.forEach((line, k) => p.label(line, cx, top + 6 + lineH * (k + 0.5), style, 'center', alpha))
  return h
}

/**
 * The panels of text: status pills, prompts, a seat's keys, the scenario's notices, the kill feed, the
 * scoreboard, death and the outcome. It remembers when things last changed, so news can be shown in full
 * and then made smaller.
 */
export class Panels {
  private seatKey = ''
  private seatSince = 0
  private objective = ''
  private objectiveSince = 0
  private backpack = ''
  private backpackSince = 0

  draw(p: Painter, s: HudStats, now: number) {
    const pillsBottom = status(p, s)
    if (s.prompt) {
      const text = s.touch ? touchPrompt(s.prompt) : s.prompt
      const style: TextStyle = { size: 16, weight: 800, color: INK }
      const w = p.measure(text, style) + 36
      const y = p.height * 0.82 - 36
      p.pill(p.width / 2 - w / 2, y, w, 36, 0xffffff, 0.9)
      p.label(text, p.width / 2, y + 18, style, 'center')
    }
    this.hint(p, s, now)
    if (s.dead && !s.notices.outcome) {
      p.rect(0, 0, p.width, p.height, 0x961414, 0.35)
      title(p, 'ELIMINATED', `Respawning in ${Math.ceil(s.respawnIn)}`)
    }
    this.scenario(p, s, now, pillsBottom + 10)
    if (s.notices.outcome) {
      const won = s.notices.outcome.won
      p.rect(0, 0, p.width, p.height, won ? 0x147850 : 0x961414, 0.3)
      title(p, won ? 'VICTORY' : 'DEFEAT', s.notices.outcome.text)
    }
    // On a touch screen the minimap's corner is the gauges', so the feed goes under the pills.
    feed(p, s, s.touch ? pillsBottom + 10 : miniBottom(p.width, p.height) + 10)
    if (s.board.length) board(p, s)
  }

  /** A seat's keys at the bottom: in full when the seat is new, then faded; above the hotbar when it is shown. */
  private hint(p: Painter, s: HudStats, now: number) {
    const hint = !s.touch ? seatHint(s.seat, s) : null
    const key = `${s.seat}|${s.canSwitchSeat}`
    if (key !== this.seatKey) {
      this.seatKey = key
      this.seatSince = now
    }
    if (!hint) return
    const style: TextStyle = { size: TEXT.sm, weight: 700, color: INK }
    const lines = wrap((t) => p.measure(t, style), hint, Math.max(240, Math.min(HINT_WIDTH, p.width - 2 * (MARGIN + CORNER_WIDTH))))
    const lineH = Math.ceil(style.size * 1.3)
    const h = lines.length * lineH + 12
    // The hotbar's slots stand 44 px off the bottom; its progress bar a little above them.
    const bottom = s.hands ? p.height - 44 - 16 : p.height - 12
    block(p, lines, p.width / 2, bottom - h, style, 0xffffff, 0.85, null, hintAlpha(now - this.seatSince))
  }

  /** Under the top pills, centred: warnings, the objective, who has the backpack, and the scenario's messages. */
  private scenario(p: Painter, s: HudStats, now: number, top: number) {
    if (s.notices.objective !== this.objective) {
      this.objective = s.notices.objective
      this.objectiveSince = now
    }
    const carried = s.backpack ? backpackLine(s.backpack) : ''
    if (carried !== this.backpack) {
      this.backpack = carried
      this.backpackSince = now
    }
    const width = p.width - 2 * MARGIN
    let y = top
    const put = (text: string, style: TextStyle, fill: number, fillAlpha: number, ring: number | null) => {
      y += block(p, wrap((t) => p.measure(t, style), text, width - 28), p.width / 2, y, style, fill, fillAlpha, ring) + 4
    }
    if (s.warning) put(s.warning, { size: 14, weight: 700, color: INK }, 0xffe066, 1, null)
    if (s.notices.objective) {
      const fresh = now - this.objectiveSince < OBJECTIVE_MS
      put(s.notices.objective, { size: fresh ? TEXT.md : TEXT.sm, weight: fresh ? 800 : 700, color: INK }, 0xffffff, fresh ? 0.85 : 0.7, TEAL)
    }
    if (s.backpack) {
      const yours = s.backpack.yours
      const fresh = now - this.backpackSince < FRESH_MS
      put(fresh ? carried : backpackShort(s.backpack), { size: fresh ? 16 : TEXT.sm, weight: 700, color: yours ? '#fff' : INK }, yours ? ORANGE : 0xffffff, yours ? 1 : 0.85, ORANGE)
    }
    for (const text of s.notices.messages) put(text, { size: 14, weight: 700, color: INK }, 0xffffff, 0.85, null)
  }
}

function title(p: Painter, heading: string, line: string) {
  const size = Math.max(32, Math.min(72, p.width * 0.08))
  const cy = p.height / 2
  p.label(heading, p.width / 2, cy - size * 0.35, { size, weight: 800, color: '#fff', shadow: true, tracking: 0.08 }, 'center')
  p.label(line, p.width / 2, cy + size * 0.45, { size: 20, weight: 700, color: '#fff', shadow: true }, 'center')
}

/**
 * The top-left pills: the connection and the voice; with the network numbers on (F3), also ping, players,
 * the tally and the sync line. Returns where the pills end, downwards.
 */
function status(p: Painter, s: HudStats): number {
  const pills: { text: string; color: string; weight?: number; alpha?: number }[] = []
  const state = s.status === 'CONNECTED' ? 'ONLINE' : s.status === 'CONNECTING' ? 'CONNECTING' : 'SOLO'
  pills.push({ text: state, color: s.status === 'CONNECTED' ? GREEN_CSS : s.status === 'CONNECTING' ? '#b26b00' : '#b3261e' })
  if (s.netStats && s.rtt !== null) pills.push({ text: `${Math.round(s.rtt)} ms`, color: INK })
  if (s.netStats) pills.push({ text: `${s.players} ${s.players === 1 ? 'player' : 'players'} in view`, color: INK })
  if (s.status === 'CONNECTED' && s.voice.mic !== 'unavailable') {
    const mic = s.voice.mic === 'denied' ? 'mic blocked' : s.voice.talking ? 'talking' : s.touch ? '' : 'V to talk'
    const peers = s.voice.peers > 0 ? `${s.voice.peers} in earshot` : ''
    const text = [mic, peers].filter(Boolean).join(' · ')
    if (text && (s.netStats || s.voice.talking || s.voice.mic === 'denied' || s.voice.peers > 0)) pills.push({ text, color: s.voice.talking ? GREEN_CSS : INK })
  }
  if (s.netStats && s.status === 'CONNECTED') pills.push({ text: `${s.kills} kills · ${s.deaths} deaths`, color: INK })
  if (s.netStats) pills.push({ text: `tiles ${s.tiles} · interp ${Math.round(s.interpDelayMs)} ms · fix ${s.correction.toFixed(2)} m · unacked ${s.pending}`, color: INK, weight: 600 })
  // Wrap before the minimap, which a touch screen does without.
  const limit = s.touch ? p.width - 12 : p.width - 12 - (miniSize(p.width, p.height) + 4) - 12
  let x = 12
  let y = 12
  for (const pill of pills) {
    const style: TextStyle = { size: TEXT.sm, weight: pill.weight ?? 700, color: pill.color }
    const w = p.measure(pill.text, style) + 20
    if (x > 12 && x + w > limit) {
      x = 12
      y += 32
    }
    p.pill(x, y, w, 24, 0xffffff, 0.85 * (pill.alpha ?? 1))
    p.label(pill.text, x + 10, y + 12, style, 'left', pill.alpha ?? 1)
    x += w + 8
  }
  return y + 24
}

/** The kill feed from `top` down, oldest first: on the right under the minimap, or on a touch screen on the left. */
function feed(p: Painter, s: HudStats, top: number) {
  const style: TextStyle = { size: TEXT.sm, weight: 700, color: INK }
  const mine: TextStyle = { ...style, weight: 900 }
  let y = top
  for (const entry of s.feed) {
    const pieces: Piece[] = []
    if (entry.killer !== 0) {
      pieces.push({ dot: colorFor(entry.killer) }, { text: who(entry.killer), style: entry.you === 'killer' ? mine : style })
      if (isBot(entry.killer) && !isZombie(entry.killer)) pieces.push({ tag: 'BOT' })
      if (entry.you === 'killer') pieces.push({ tag: 'YOU' })
      pieces.push({ text: `  ${mark(entry.weapon)}  `, style: { ...style, weight: 800 } })
    }
    pieces.push({ dot: colorFor(entry.victim) }, { text: who(entry.victim), style: entry.you === 'victim' ? mine : style })
    if (isBot(entry.victim)) pieces.push({ tag: 'BOT' })
    if (entry.you === 'victim') pieces.push({ tag: 'YOU' })
    if (entry.killer === 0) pieces.push({ text: entry.weapon === WEAPON_DROWN ? ' drowned' : ` took ${who(0)}`, style })
    const w = pieces.reduce((sum, piece) => sum + pieceWidth(p, piece), 0) + 20
    const x = s.touch ? 12 : p.width - 12 - w
    const ring = entry.you === 'killer' ? TEAL : entry.you === 'victim' ? RED : null
    p.pill(x, y, w, 24, 0xffffff, 0.85, ring === null ? 0 : 2, ring ?? 0, 1)
    drawPieces(p, pieces, x + 10, y + 12)
    y += 28
  }
}

/** The scoreboard, while Tab is held: every player by side. */
function board(p: Painter, s: HudStats) {
  const head: TextStyle = { size: TEXT.sm, weight: 700, color: 'rgba(29, 43, 54, 0.7)' }
  const sideStyle: TextStyle = { size: TEXT.sm, weight: 700, color: INK }
  const cell = (you: boolean): TextStyle => ({ size: 16, weight: you ? 800 : 700, color: you ? GREEN_CSS : INK })
  const columns = s.backpack ? ['Kills', 'Deaths', 'Backpack'] : ['Kills', 'Deaths']
  const nameOf = (id: number, you: boolean) => (you ? `${nameFor(id)} (you)` : nameFor(id))
  let nameWidth = p.measure('Player', head)
  for (const side of s.board) for (const row of side.rows) nameWidth = Math.max(nameWidth, 18 + p.measure(nameOf(row.id, row.you), cell(row.you)) + (isBot(row.id) ? p.measure('BOT', TAG) + 16 : 0))
  const colWidth = 76
  const width = Math.max(320, 16 * 2 + nameWidth + 16 + columns.length * colWidth)
  const rowH = 28
  const rows = s.board.reduce((n, side) => n + side.rows.length + (s.board.length > 1 ? 1 : 0), 1)
  // A long board stops at the screen's edge, with a line for those left out.
  const room = Math.max(3, Math.floor((p.height - 2 * MARGIN - 20) / rowH))
  const more = rows > room
  const players = s.board.reduce((n, side) => n + side.rows.length, 0)
  let drawn = 0
  const height = 20 + Math.min(rows, room) * rowH + (s.board.length > 1 ? s.board.length * 6 : 0)
  const x = p.width / 2 - width / 2
  let y = Math.max(MARGIN, p.height / 2 - height / 2)
  const bottom = y + height - 10 - (more ? rowH : 0)
  p.box(x, y, width, height, 14, 0xffffff, 0.92)
  const right = (k: number) => x + width - 16 - (columns.length - 1 - k) * colWidth
  y += 10
  p.label('Player', x + 16, y + rowH / 2, head)
  columns.forEach((column, k) => p.label(column, right(k), y + rowH / 2, head, 'right'))
  y += rowH
  for (const side of s.board) {
    if (y + rowH > bottom + 0.5) break
    if (s.board.length > 1) {
      y += 6
      p.label(sideName(side.team), x + 16, y + rowH / 2, sideStyle)
      const values = [String(side.kills), String(side.deaths), clock(side.held)]
      columns.forEach((_, k) => p.label(values[k]!, right(k), y + rowH / 2, sideStyle, 'right'))
      p.rect(x + 16, y + rowH - 1, width - 32, 1, 0x1d2b36, 0.15)
      y += rowH
    }
    for (const row of side.rows) {
      if (y + rowH > bottom + 0.5) break
      const style = cell(row.you)
      const pieces: Piece[] = [{ dot: colorFor(row.id) }, { text: nameOf(row.id, row.you), style }]
      if (isBot(row.id)) pieces.push({ tag: 'BOT' })
      drawPieces(p, pieces, x + 16, y + rowH / 2)
      const values = [String(row.kills), String(row.deaths), clock(row.held)]
      columns.forEach((_, k) => p.label(values[k]!, right(k), y + rowH / 2, style, 'right'))
      y += rowH
      drawn++
    }
  }
  if (more) p.label(`+${players - drawn} more`, x + 16, y + rowH / 2, head)
}
