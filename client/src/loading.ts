/*
 * What the game is doing between the page opening and its first frame (LOAD-TIME.md): the stages of
 * `Game.start`, for the loading screen to list and to fill its bar by, and the time each took, left
 * in the page's performance timeline as `load.<stage>` measures for `scripts/load-bench.mjs`.
 */

/** A stage of the start that the loading screen names. Several run at once. */
export type StageKey = 'renderer' | 'world' | 'physics' | 'soldier' | 'terrain' | 'buildings'

export type StageState = 'waiting' | 'running' | 'done' | 'failed'

export interface LoadStage {
  key: StageKey
  /** What the loading screen calls it. */
  label: string
  state: StageState
  /** How many of its `of` parts are done (tiles, for the terrain); `of` is 0 for a stage with no count. */
  done: number
  of: number
}

export interface LoadProgress {
  stages: readonly LoadStage[]
  /** How much of the start is behind us, 0..1. It never goes back. */
  fraction: number
  /** What is still arriving behind the first frame (the vehicles' models), or null. */
  background: string | null
}

/**
 * Each stage's share of the bar: roughly its share of a cold start over a home connection, as
 * `scripts/load-bench.mjs --down 20 --latency 40` measured it on the umea world (LOAD-TIME.md). The
 * stages overlap, so the shares are of the work, not of the wait.
 */
export const STAGE_WEIGHTS: Record<StageKey, number> = {
  renderer: 0.02,
  world: 0.03,
  physics: 0.3,
  soldier: 0.3,
  terrain: 0.33,
  buildings: 0.02,
}

const LABELS: Record<StageKey, string> = {
  renderer: 'Starting the renderer',
  world: 'Reading the world',
  physics: 'Starting the physics',
  soldier: 'Loading the soldier',
  terrain: 'Building the terrain',
  buildings: 'Making the buildings solid',
}

/** The stages in the order the loading screen lists them, none started. */
export function freshStages(): LoadStage[] {
  return (Object.keys(LABELS) as StageKey[]).map((key) => ({ key, label: LABELS[key], state: 'waiting', done: 0, of: 0 }))
}

/** How much of the start `stages` have behind them, 0..1: a counted stage fills by its count. */
export function fractionOf(stages: readonly LoadStage[]): number {
  let sum = 0
  for (const stage of stages) {
    const share = stage.state === 'done' ? 1 : stage.state === 'running' && stage.of > 0 ? Math.min(1, stage.done / stage.of) : 0
    sum += STAGE_WEIGHTS[stage.key] * share
  }
  return Math.min(1, sum)
}

/** Leaves `load.<name>` in the performance timeline: `start` to now, ms since the page opened. */
function measured(name: string, start: number) {
  performance.measure?.(`load.${name}`, { start, end: performance.now() })
}

/** Follows the start of the game: tells `onChange` each time a stage moves, and times every stage. */
export class LoadTracker {
  private readonly stages = freshStages()
  private fraction = 0
  private background: string | null = null

  constructor(private readonly onChange: (progress: LoadProgress) => void = () => {}) {}

  /** Where the start is now. */
  get progress(): LoadProgress {
    this.fraction = Math.max(this.fraction, fractionOf(this.stages))
    return { stages: this.stages.map((stage) => ({ ...stage })), fraction: this.fraction, background: this.background }
  }

  /** Runs `work` as stage `key`: running until it settles, then done or failed, and timed. */
  async run<T>(key: StageKey, work: () => Promise<T> | T): Promise<T> {
    const start = performance.now()
    this.set(key, 'running')
    try {
      const result = await work()
      this.set(key, 'done')
      return result
    } catch (error) {
      this.set(key, 'failed')
      throw error
    } finally {
      measured(key, start)
    }
  }

  /** Stage `key` has `done` of its `of` parts behind it. */
  count(key: StageKey, done: number, of: number) {
    const stage = this.stage(key)
    if (stage.done === done && stage.of === of) return
    stage.done = done
    stage.of = of
    this.onChange(this.progress)
  }

  /** Times `work` as `load.<name>` without listing it: what the loading screen does not name. */
  async time<T>(name: string, work: () => Promise<T> | T): Promise<T> {
    const start = performance.now()
    try {
      return await work()
    } finally {
      measured(name, start)
    }
  }

  /** Marks the moment `name` as `load.<name>`, measured from the page's opening. */
  reached(name: string) {
    measured(name, 0)
  }

  /** Says what is still arriving behind the first frame, until `work` settles. */
  behind(label: string, work: Promise<unknown>) {
    this.background = label
    this.onChange(this.progress)
    const over = () => {
      this.background = null
      this.onChange(this.progress)
    }
    work.then(over, over)
  }

  private stage(key: StageKey): LoadStage {
    return this.stages.find((stage) => stage.key === key)!
  }

  private set(key: StageKey, state: StageState) {
    this.stage(key).state = state
    this.onChange(this.progress)
  }
}
