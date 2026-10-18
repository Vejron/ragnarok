import { describe, expect, it } from 'vitest'
import { NOTICE_LOST, NOTICE_OBJECTIVE, NOTICE_SAY, NOTICE_WON } from '@/net/codec'
import { MESSAGE_MS, MESSAGES, Notices, OUTCOME_MS } from '@/notices'

describe('scenario notices', () => {
  it('keeps the objective until the next one and lets messages fade, the newest few at a time', () => {
    const notices = new Notices()
    notices.receive(NOTICE_OBJECTIVE, 'Clear the farm', 0)
    for (let n = 1; n <= MESSAGES + 1; n++) notices.receive(NOTICE_SAY, `message ${n}`, n)
    expect(notices.view(10)).toEqual({ objective: 'Clear the farm', messages: ['message 2', 'message 3', 'message 4'], outcome: null })
    expect(notices.view(MESSAGE_MS + 10).messages).toEqual([])
    expect(notices.view(MESSAGE_MS + 10).objective).toBe('Clear the farm')
    notices.receive(NOTICE_OBJECTIVE, '', 20_000)
    expect(notices.view(20_000).objective).toBe('')
  })

  it('shows a win or a loss for a while, and the objective goes with it', () => {
    const notices = new Notices()
    notices.receive(NOTICE_OBJECTIVE, 'Hold the bridge', 0)
    notices.receive(NOTICE_WON, 'Held', 100)
    expect(notices.view(200)).toEqual({ objective: '', messages: [], outcome: { won: true, text: 'Held' } })
    expect(notices.view(100 + OUTCOME_MS).outcome).toBeNull()
    notices.receive(NOTICE_LOST, 'Out of time', 50_000)
    expect(notices.view(50_000).outcome).toEqual({ won: false, text: 'Out of time' })
  })
})
