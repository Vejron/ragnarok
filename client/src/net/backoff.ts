/**
 * Delays between reconnect attempts: doubling from `baseMs` up to `maxMs`, each spread by up to
 * 25 per cent either way so that clients dropped together (a server restart) do not come back
 * together. `reset` after a connection has worked.
 */
export class Backoff {
  private attempts = 0

  constructor(
    private readonly baseMs = 1000,
    private readonly maxMs = 30_000,
    private readonly random: () => number = Math.random,
  ) {}

  next(): number {
    const delay = Math.min(this.maxMs, this.baseMs * 2 ** this.attempts)
    this.attempts = Math.min(this.attempts + 1, 16)
    return Math.round(delay * (0.75 + this.random() * 0.5))
  }

  reset() {
    this.attempts = 0
  }
}
