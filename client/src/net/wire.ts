/**
 * What the generated decoders (`messages.generated.ts`) are made of: one reader per kind of field, each
 * the value as sent, or `undefined` if it is not one.
 */

import { decodeUpdates, type Update } from './replicated'

export const int = (raw: unknown): number | undefined => (Number.isInteger(raw) ? (raw as number) : undefined)
export const bool = (raw: unknown): boolean | undefined => (typeof raw === 'boolean' ? raw : undefined)
export const text = (raw: unknown): string | undefined => (typeof raw === 'string' ? raw : undefined)

/** A list of integers, of `length` when it is given. */
export function ints(raw: unknown, length?: number): number[] | undefined {
  if (!Array.isArray(raw) || (length !== undefined && raw.length !== length)) return undefined
  for (const item of raw) if (!Number.isInteger(item)) return undefined
  return raw as number[]
}

/** A list of `item`s. */
export function list<T>(raw: unknown, item: (raw: unknown) => T | undefined): T[] | undefined {
  if (!Array.isArray(raw)) return undefined
  const items: T[] = []
  for (const each of raw) {
    const value = item(each)
    if (value === undefined) return undefined
    items.push(value)
  }
  return items
}

/** Nothing (`null`), or `some`. */
export const optional = <T>(raw: unknown, some: (raw: unknown) => T | undefined): T | null | undefined => (raw === null ? null : some(raw))

/** A snapshot's entity updates (`replicated.ts`). */
export const updates = (raw: unknown): Update[] | undefined => decodeUpdates(raw) ?? undefined
