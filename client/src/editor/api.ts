import type { BlockStatus, TileId } from './grid'

export interface WorldSummary {
  name: string
  tiles: number
  active: boolean
  generated: boolean
}

export interface Job {
  world: string
  kind: 'download' | 'generate'
  state: 'idle' | 'running' | 'done' | 'failed'
  done: number
  total: number
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
  world: async (name: string) => (await call<{ tiles: TileId[] }>(`/worlds/${name}`)).tiles,
  save: (name: string, tiles: TileId[]) => call(`/worlds/${name}`, json('PUT', { tiles })),
  remove: (name: string) => call(`/worlds/${name}`, { method: 'DELETE' }),
  activate: (name: string) => call(`/worlds/${name}/activate`, { method: 'POST' }),
  generate: (name: string) => call<Job>(`/worlds/${name}/generate`, { method: 'POST' }),
  fetchWorld: (name: string) => call<Job>(`/worlds/${name}/fetch`, { method: 'POST' }),
  job: () => call<Job>('/job'),
  cache: async (south: number, west: number, north: number, east: number) =>
    (await call<{ blocks: BlockStatus[] }>(`/cache?south=${south}&west=${west}&north=${north}&east=${east}`)).blocks,
}
