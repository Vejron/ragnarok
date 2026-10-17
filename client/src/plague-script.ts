/**
 * Everything the Orange Plague says, by what he is talking about. This file imports nothing, so
 * `tools/voice/plague_voice.mjs` can read it to record the lines (`client/public/voice/plague/`).
 *
 * `text` is the subtitle; `{name}` in it is the player the line is about. A recording cannot know
 * that name, so a line with one has `say`: the same line as recorded, without it. `mood` is how it is
 * delivered, by the recording and by the browser's voice when there is no recording.
 */

export type Mood = 'boast' | 'smug' | 'angry' | 'pain' | 'whine'

export interface Line {
  text: string
  say?: string
  mood: Mood
}

export const SCRIPT = {
  arrive: [
    { text: "Folks... I'm here. Tremendous city. Very low energy. We're going to fix it, believe me.", mood: 'boast' },
    { text: 'Nice town. Small. Very small. I could buy it twice before lunch.', mood: 'smug' },
    { text: "I've been looking at this downtown. Condos! Gold towers! The people want it!", mood: 'boast' },
  ],
  stomp: [
    { text: 'Corporate restructuring!', mood: 'angry' },
    { text: 'Total teardown!', mood: 'angry' },
    { text: "Watch the shoes. They're Italian!", mood: 'smug' },
    { text: 'Out of my way, losers!', mood: 'angry' },
  ],
  strike: [
    { text: 'Get me the Pentagon. No... the good one.', mood: 'smug' },
    { text: 'Hello? Yes. That one. Hit it. Hit it hard.', mood: 'angry' },
    { text: "{name}! You're on my list, {name}. Very bad list.", say: "You! You're on my list. Very bad list.", mood: 'angry' },
    { text: 'Send everything to {name}. Everything!', say: 'Send everything over there. Everything!', mood: 'angry' },
  ],
  hurt: [
    { text: 'Fake weapons! Tremendously weak shells!', mood: 'angry' },
    { text: "That didn't even dent the suit. Pure wool. Custom made!", mood: 'smug' },
    { text: "Is that all you've got, {name}? Sad!", say: "Is that all you've got? Sad!", mood: 'smug' },
    { text: 'The media said you had big firepower. Fake news!', mood: 'angry' },
  ],
  fired: [
    { text: "You're fired, {name}!", say: "You're fired!", mood: 'boast' },
    { text: '{name}, clear your desk. Out!', say: 'Clear your desk. Out!', mood: 'angry' },
    { text: "Unbelievable failure, {name}. I've seen better strategy from a golf caddie.", say: "Unbelievable failure. I've seen better strategy from a golf caddie.", mood: 'smug' },
    { text: '{name}? Never heard of them. Total loser.', say: 'Who was that? Never heard of them. Total loser.', mood: 'smug' },
  ],
  adaptInfantry: [
    { text: 'Little people with little guns. I see you down there!', mood: 'smug' },
    { text: "All these tiny soldiers. Somebody step on them. I'll do it myself.", mood: 'angry' },
  ],
  adaptArmor: [
    { text: 'Tanks? Very weak tanks. Sad!', mood: 'smug' },
    { text: "Nice tanks. I'll take them. Scrap value... very low.", mood: 'smug' },
  ],
  adaptAir: [
    { text: 'Helicopters. Very annoying. Like flies, folks. Like flies.', mood: 'whine' },
    { text: "Whoever's flying that thing... you're grounded. Permanently.", mood: 'angry' },
  ],
  rezone: [
    { text: 'Rezoned! Beautiful. Gold tower coming soon.', mood: 'boast' },
    { text: 'Sold! Best deal in the history of deals.', mood: 'boast' },
    { text: 'Low-quality building. Made by losers. Gone!', mood: 'smug' },
  ],
  idle: [
    { text: "People come up to me, big strong generals, tears in their eyes: 'Sir, nobody stomps a city like you.'", mood: 'boast' },
    { text: "These bridges? Weak! We're going to build something much bigger. Much better.", mood: 'boast' },
    { text: "We're going to rezone this whole district. Luxury retail. The people want it.", mood: 'smug' },
    { text: 'I have the best words. The best stomps. Everybody says so.', mood: 'boast' },
  ],
  down: [
    { text: 'Rigged... total witch hunt... see you in court, {name}...!', say: 'Rigged... total witch hunt... see you in court...!', mood: 'pain' },
    { text: "Rigged! It's rigged! I'm taking this city to the Supreme Court!", mood: 'pain' },
  ],
  leave: [
    { text: 'Total victory! Huge ratings! Best city destruction in history!', mood: 'boast' },
    { text: "I've seen enough. I'll take it all. Bill the city!", mood: 'smug' },
  ],
  nemesis: [
    { text: '{name}. Who is {name}? A nobody. A total nobody. Enemy of the Week!', say: 'Who is this person? A nobody. A total nobody. Enemy of the Week!', mood: 'angry' },
    { text: '{name}! Very nasty. Very unfair. Enemy of the Week, folks!', say: 'Very nasty. Very unfair. Enemy of the Week, folks!', mood: 'angry' },
  ],
  eye: [
    { text: 'My eye! You shot my eye, {name}! Very unfair. Very, very unfair!', say: 'My eye! You shot my eye! Very unfair. Very, very unfair!', mood: 'pain' },
    { text: "I lost an eye, folks. Doesn't matter. I still see everything. Everything!", mood: 'angry' },
    { text: 'Nobody touches the eyes! Beautiful eyes. The best eyes. {name}, you are finished!', say: 'Nobody touches the eyes! Beautiful eyes. The best eyes. You are finished!', mood: 'pain' },
  ],
  blind: [
    { text: "I can't see! Fake darkness! Somebody turn the lights back on!", mood: 'pain' },
    { text: 'Total blackout, folks. Total. Rigged!', mood: 'whine' },
  ],
  ear: [
    { text: "What? WHAT? Speak up, {name}, I can't hear you!", say: "What? WHAT? Speak up, I can't hear you!", mood: 'angry' },
    { text: 'My ear! Very expensive ear. The best ear. Gone!', mood: 'pain' },
  ],
  gagged: [
    { text: "You can't silence me! You can't sil-", mood: 'angry' },
    { text: 'My mouth! My beautiful, beautiful mou-', mood: 'pain' },
  ],
} satisfies Record<string, readonly Line[]>

export type Topic = keyof typeof SCRIPT

/** The file a line is recorded in, under `voice/plague/`, without its extension. */
export function clipName(topic: Topic, index: number) {
  return `${topic}_${index + 1}`
}
