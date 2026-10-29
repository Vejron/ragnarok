import { Character, type Crowd } from '@/render/character'

/**
 * Every player's soldier, by player, for whoever draws it this frame: the local player's own camera,
 * the soldiers presenter for those on foot, and the vehicles for those in a seat. A player keeps one
 * soldier as they get in and out, so it is shared rather than owned by one presenter, and one not
 * drawn in a frame is let go at the end of it.
 *
 * The local player's soldier is kept under key 0, from before the server gives them an id.
 */
export class Avatars {
  private readonly all = new Map<number, Character>()
  private readonly drawnNow = new Set<number>()
  /** The local player's id, once the server has given one. */
  localId: number | null = null

  constructor(private readonly crowd: Crowd) {}

  /** The key player `id`'s soldier is kept under. */
  private key(id: number) {
    return id === this.localId ? 0 : id
  }

  /** Player `id`'s soldier, if it has one. */
  of(id: number): Character | undefined {
    return this.all.get(this.key(id))
  }

  /** The local player's soldier, made if need be and drawn this frame. */
  local(): Character {
    return this.draw(0)
  }

  /** Player `id`'s soldier, made if need be, to be drawn this frame. */
  draw(id: number): Character {
    const key = this.key(id)
    let avatar = this.all.get(key)
    if (!avatar) {
      avatar = new Character(this.crowd, key === 0 ? (this.localId ?? 0) : key)
      this.all.set(key, avatar)
    }
    this.drawnNow.add(key)
    return avatar
  }

  /** The keys drawn so far this frame: 0 for the local player, else player ids. */
  get drawn(): ReadonlySet<number> {
    return this.drawnNow
  }

  /** Starts a frame. The local player's soldier is always kept. */
  begin() {
    this.drawnNow.clear()
    this.drawnNow.add(0)
  }

  /** Lets go of every soldier not drawn this frame, telling `gone` each key. */
  sweep(gone: (key: number) => void) {
    for (const [key, avatar] of this.all) {
      if (this.drawnNow.has(key)) continue
      gone(key)
      avatar.dispose()
      this.all.delete(key)
    }
  }

  /** Lets go of the local player's soldier, to be made again in their colour. */
  forgetLocal() {
    this.all.get(0)?.dispose()
    this.all.delete(0)
  }

  /** Lets go of everyone's but the local player's: a disconnect. */
  forgetOthers() {
    for (const [key, avatar] of this.all) {
      if (key === 0) continue
      avatar.dispose()
      this.all.delete(key)
    }
  }

  /** Each soldier with its key. */
  entries() {
    return this.all.entries()
  }
}
