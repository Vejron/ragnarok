import type { BlockStatus, TileId } from './grid'
import type { Placements } from './placements'
import type { Scenario } from './scenario'

export interface WorldSummary {
  name: string
  tiles: number
  active: boolean
  generated: boolean
}

/** What the importer is doing: the stage, its steps, and the bytes and rate so far. */
export interface Progress {
  stage: string
  step: number
  steps: number
  bytes: number
  bytesPerSecond: number
  cached: number
  cachedPerSecond: number
}

export interface Job {
  world: string
  kind: 'download' | 'generate'
  state: 'idle' | 'running' | 'done' | 'failed'
  /** Blocks downloaded, or 0 and 1 for a generate. */
  done: number
  total: number
  progress: Progress
  elapsedMs: number
  messages: string[]
}

async function call<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`/api${path}`, init)
  const body = await response.json()
  if (!response.ok) throw new Error(body.error ?? response.statusText)
  return body as T
}

const json = (method: string, body: unknown): RequestInit => ({
  method,
  headers: { 'content-type': 'application/json' },
  body: JSON.stringify(body),
})

export const api = {
  grid: () => call<import('./grid').GridInfo>('/grid'),
  worlds: async () => (await call<{ worlds: WorldSummary[] }>('/worlds')).worlds,
  world: (name: string) => call<{ tiles: TileId[] } & Partial<Placements>>(`/worlds/${name}`),
  save: (name: string, tiles: TileId[], placements: Placements) => call(`/worlds/${name}`, json('PUT', { tiles, ...placements })),
  remove: (name: string) => call(`/worlds/${name}`, { method: 'DELETE' }),
  activate: (name: string) => call(`/worlds/${name}/activate`, { method: 'POST' }),
  generate: (name: string) => call<Job>(`/worlds/${name}/generate`, { method: 'POST' }),
  fetchWorld: (name: string) => call<Job>(`/worlds/${name}/fetch`, { method: 'POST' }),
  job: () => call<Job>('/job'),
  /** The world's scenario, or undefined when it has none. */
  scenario: async (world: string): Promise<Scenario | undefined> => {
    const response = await fetch(`/api/worlds/${world}/scenario`)
    if (response.status === 404) return undefined
    const body = await response.json()
    if (!response.ok) throw new Error(body.error ?? response.statusText)
    return body as Scenario
  },
  saveScenario: (world: string, scenario: Scenario) => call(`/worlds/${world}/scenario`, json('PUT', scenario)),
  removeScenario: (world: string) => call(`/worlds/${world}/scenario`, { method: 'DELETE' }).catch(() => undefined),
  cache: async (south: number, west: number, north: number, east: number) =>
    (await call<{ blocks: BlockStatus[] }>(`/cache?south=${south}&west=${west}&north=${north}&east=${east}`)).blocks,
}
