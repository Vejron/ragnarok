import { describe, expect, it } from 'vitest'
import { clockOf, DeliveryTracker, houseName, shopName, type JobOffer, type JobState } from '@/deliveries'
import { JOB_CARRY, JOB_FETCH, JOB_LOST } from '@/sim/consts'

const offer = (id: number): JobOffer => ({ id, shopX: 10, shopZ: 20, houseX: 300, houseZ: -40, pay: 50, par: 120 })
const job = (id: number, stage: number, left = 100): JobState => ({ id, stage, x: 1, z: 2, pay: 50, left })
const nameOf = (id: number) => `P${id}`

describe('DeliveryTracker', () => {
  it('says nothing of how things stood when first told, and is active from then', () => {
    const t = new DeliveryTracker()
    expect(t.view()).toBeNull()
    expect(t.receive([offer(1)], [job(2, JOB_CARRY)])).toBeNull()
    expect(t.active).toBe(true)
    expect(t.view(30)?.cash).toBe(30)
  })

  it('tells the courier each step of their own job', () => {
    const t = new DeliveryTracker()
    t.receive([offer(1)], [])
    expect(t.receive([], [job(1, JOB_FETCH)])).toBe(`Order taken: pick it up at ${shopName(1)}`)
    expect(t.receive([], [job(1, JOB_CARRY)])).toBe(`Got it! Deliver to ${houseName(1)}`)
    expect(t.receive([], [job(1, JOB_CARRY, 99)])).toBeNull()
    expect(t.receive([], [job(1, JOB_LOST)])).toBe('You dropped the order!')
    expect(t.receive([], [])).toBe(`The order for ${houseName(1)} spoiled`)
  })

  it('tells a robber whose order they grabbed', () => {
    const t = new DeliveryTracker()
    t.receive([], [])
    expect(t.receive([], [job(7, JOB_CARRY)])).toContain(houseName(7))
  })

  it('takes a job gone after a delivery for delivered, not spoiled', () => {
    const t = new DeliveryTracker()
    t.receive([], [job(1, JOB_LOST)])
    expect(t.delivered({ by: 4, courier: 3, job: 1, pay: 40, cash: 90 }, 3, nameOf)).toBe('P4 delivered your order and took the 40 kr')
    expect(t.receive([], [])).toBeNull()
  })

  it('pays the one who hands it in, and says whose it was', () => {
    const t = new DeliveryTracker()
    expect(t.delivered({ by: 3, courier: 3, job: 1, pay: 40, cash: 90 }, 3, nameOf)).toBe('Delivered! +40 kr')
    expect(t.delivered({ by: 3, courier: 5, job: 2, pay: 25, cash: 115 }, 3, nameOf)).toBe("Delivered P5's order! +25 kr")
    expect(t.delivered({ by: 6, courier: 5, job: 3, pay: 25, cash: 25 }, 3, nameOf)).toBeNull()
  })

  it('points the waypoint at the parcel in hand before a job still to fetch', () => {
    const t = new DeliveryTracker()
    t.receive([], [{ ...job(1, JOB_FETCH), x: 5, z: 6 }, { ...job(2, JOB_CARRY), x: 7, z: 8 }])
    expect(t.view()?.target).toEqual({ x: 7, z: 8, stage: JOB_CARRY })
    t.clear()
    expect(t.view()).toBeNull()
  })

  it('names each job the same everywhere, and shows its clock as m:ss', () => {
    expect(shopName(12)).toBe(shopName(12))
    expect(houseName(12)).toMatch(/^the \S+s$/)
    expect(clockOf(83)).toBe('1:23')
    expect(clockOf(-5)).toBe('−0:05')
  })
})
