//! What the Orange Plague thinks: whom he has seen and where they were going, whom he hates and
//! what has been hurting him, and so which attack to make next and where to aim it.
//!
//! The brain knows nothing of the world. The mechanic hands it `Contact`s (what is out there and
//! whether he can see it) and `PlagueWound`s (who hurt him), and it hands back a `Decision`. That
//! keeps it a plain function of what it is told, so it can be tested without physics.

use crate::game::PlagueWound;
use shared::consts::*;
use std::collections::BTreeMap;

/// How the brain thinks about a contact, and about whoever hurt him from it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Category {
    /// On foot.
    Infantry,
    /// In a vehicle on the ground.
    Armor,
    /// In a helicopter, a plane or a drone, off the ground.
    Air,
}

const CATEGORIES: usize = 3;

impl Category {
    fn index(self) -> usize {
        self as usize
    }
}

/// Something out there worth stomping or striking: a stickman, or a vehicle with people in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Contact {
    /// The player's id, or the vehicle's.
    pub id: u32,
    pub position: [f32; 3],
    pub category: Category,
    /// The players it is: the stickman themselves, or everyone in the vehicle.
    pub crew: Vec<u32>,
    /// Whether he can see it from his head right now.
    pub visible: bool,
}

/// What he remembers of a contact: where it was last seen and how it was moving.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Sighting {
    position: [f32; 3],
    velocity: [f32; 3],
    category: Category,
    /// When it was last seen, in ticks.
    tick: u32,
    visible: bool,
}

/// What he does next.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Decision {
    /// Nothing worth an attack: carry on with the agenda.
    Walk,
    /// Stomp on this point, near his feet.
    Stomp([f32; 3]),
    /// Call an airstrike on this point.
    Strike([f32; 3]),
}

/// The brain thinks this often, ticks: twice a second.
pub const THINK_EVERY: u32 = TICK_RATE / 2;
/// A contact out of sight is remembered this long, ticks.
const MEMORY_TICKS: u32 = 6 * TICK_RATE;
/// Two sightings further apart than this are not used for a speed: too old to say where it is going.
const TRACK_TICKS: u32 = 2 * TICK_RATE;
/// Hatred halves in this long, ticks, so whoever is hurting him now matters most.
const THREAT_HALF_LIFE_TICKS: f32 = 20.0 * TICK_RATE as f32;
/// The damage mix forgets on the same clock.
const MIX_HALF_LIFE_TICKS: f32 = 30.0 * TICK_RATE as f32;
/// After an attack, the next of the same kind waits this long, ticks.
pub const STOMP_COOLDOWN_TICKS: u32 = 3 * TICK_RATE;
pub const STRIKE_COOLDOWN_TICKS: u32 = 10 * TICK_RATE;
/// An attack has to be worth at least this much: one stickman in the open, seen, hated by nobody.
const MIN_SCORE: f32 = 1.0;
/// He would rather stomp than phone: a stomp's score is scaled by this.
const STOMP_PREFERENCE: f32 = 1.3;
/// Something remembered but out of sight counts this much of what it would in plain view: a strike
/// on where it hid might still get it, and will at least take the roof off.
const HIDDEN_WEIGHT: f32 = 0.6;
/// Aircraft dodge: they count for this much in an airstrike.
const AIR_STRIKE_WEIGHT: f32 = 0.5;
/// The longest an airstrike leads a moving target, metres.
const MAX_LEAD: f32 = 80.0;
/// Hatred it takes to be his Enemy of the Week.
const NEMESIS_MIN: f32 = 300.0;
/// Damage taken lately before he has an opinion on what is hurting him.
const ADAPT_MIN: f32 = 300.0;
/// Scores are scaled by a random factor in 1 ± this, so equal choices do not always go the same way.
const NOISE: f32 = 0.15;

/// Seconds from the call to the rockets landing, on average: the wind-up, half the barrage and the
/// fall. What an airstrike leads a moving target by.
pub fn strike_lead_seconds() -> f32 {
    let windup = PLAGUE_STRIKE_WINDUP_TICKS as f32 / TICK_RATE as f32;
    let barrage = (PLAGUE_STRIKE_ROCKETS * PLAGUE_STRIKE_INTERVAL_TICKS) as f32 / TICK_RATE as f32 / 2.0;
    windup + barrage + fall_seconds()
}

/// How long an airstrike rocket falls, seconds: `PLAGUE_STRIKE_DROP_HEIGHT` metres from
/// `PLAGUE_STRIKE_DROP_SPEED` under `GRAVITY`.
pub fn fall_seconds() -> f32 {
    let (v, g, h) = (PLAGUE_STRIKE_DROP_SPEED, GRAVITY, PLAGUE_STRIKE_DROP_HEIGHT);
    (-v + (v * v + 2.0 * g * h).sqrt()) / g
}

pub(super) fn flat_distance(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[2] - b[2])
}

#[derive(Debug, Default)]
pub struct Brain {
    sightings: BTreeMap<u32, Sighting>,
    /// Damage each player has done him, fading.
    threat: BTreeMap<u32, f32>,
    /// Damage he took from each `Category`, fading: what has been hurting him lately.
    mix: [f32; CATEGORIES],
    /// Tick at which each attack is ready again.
    stomp_ready: u32,
    strike_ready: u32,
}

impl Brain {
    /// Takes in what he saw this think: updates where everything is and how it moves, and forgets
    /// what has been out of sight too long.
    pub fn perceive(&mut self, tick: u32, contacts: &[Contact]) {
        for contact in contacts {
            let before = self.sightings.get(&contact.id).copied();
            if !contact.visible {
                // Out of sight: he keeps what he last saw, and knows it is no longer in view.
                if let Some(mut sighting) = before {
                    sighting.visible = false;
                    self.sightings.insert(contact.id, sighting);
                }
                continue;
            }
            let velocity = match before {
                Some(seen) if seen.visible && tick > seen.tick && tick - seen.tick <= TRACK_TICKS => {
                    let seconds = (tick - seen.tick) as f32 / TICK_RATE as f32;
                    std::array::from_fn(|i| (contact.position[i] - seen.position[i]) / seconds)
                }
                _ => [0.0; 3],
            };
            self.sightings.insert(contact.id, Sighting { position: contact.position, velocity, category: contact.category, tick, visible: true });
        }
        // Whatever is gone from the world (died, left, got out of the vehicle) is forgotten at once.
        self.sightings.retain(|id, seen| contacts.iter().any(|c| c.id == *id) && tick.saturating_sub(seen.tick) <= MEMORY_TICKS);
    }

    /// Takes in that `contact`, out of sight, was heard at `position` just now: where he will strike
    /// it, or stomp if it is that close.
    pub fn hear(&mut self, tick: u32, contact: &Contact, position: [f32; 3]) {
        self.sightings.insert(contact.id, Sighting { position, velocity: [0.0; 3], category: contact.category, tick, visible: false });
    }

    /// Where he thinks `contact` is right now, if he has seen it this think or just heard it: its
    /// true place in plain view, where it sounded from otherwise.
    fn now(&self, tick: u32, contact: &Contact) -> Option<[f32; 3]> {
        if contact.visible {
            return Some(contact.position);
        }
        self.sightings.get(&contact.id).filter(|s| s.tick == tick).map(|s| s.position)
    }

    /// Takes in who hurt him, as what (`category_of` says what each attacker is now).
    pub fn feel(&mut self, wounds: &[PlagueWound], category_of: impl Fn(u32) -> Option<Category>) {
        for wound in wounds {
            let Some(attacker) = wound.attacker else { continue };
            *self.threat.entry(attacker).or_default() += wound.amount as f32;
            if let Some(category) = category_of(attacker) {
                self.mix[category.index()] += wound.amount as f32;
            }
        }
    }

    /// Lets hatred and the damage mix fade by `ticks` worth.
    pub fn forget(&mut self, ticks: u32) {
        let threat = 0.5f32.powf(ticks as f32 / THREAT_HALF_LIFE_TICKS);
        for value in self.threat.values_mut() {
            *value *= threat;
        }
        self.threat.retain(|_, value| *value > 1.0);
        let mix = 0.5f32.powf(ticks as f32 / MIX_HALF_LIFE_TICKS);
        for value in &mut self.mix {
            *value *= mix;
        }
    }

    /// The player he hates most, if anyone has earned it: his Enemy of the Week.
    pub fn nemesis(&self) -> Option<u32> {
        self.threat.iter().filter(|(_, t)| **t >= NEMESIS_MIN).max_by(|a, b| a.1.total_cmp(b.1)).map(|(&id, _)| id)
    }

    /// The share of what has been hurting him lately that came from `category`, 0..=1.
    pub fn share(&self, category: Category) -> f32 {
        let total: f32 = self.mix.iter().sum();
        if total <= 0.0 { 0.0 } else { self.mix[category.index()] / total }
    }

    /// The category that has hurt him most lately, if enough has to say.
    pub fn worst(&self) -> Option<Category> {
        let total: f32 = self.mix.iter().sum();
        if total < ADAPT_MIN {
            return None;
        }
        [Category::Infantry, Category::Armor, Category::Air].into_iter().max_by(|a, b| self.mix[a.index()].total_cmp(&self.mix[b.index()]))
    }

    /// How much one contact is worth hitting: one for being there, more for the hatred its crew has
    /// earned, more for a vehicle, all scaled up by how much its kind has been hurting him.
    fn weight(&self, contact: &Contact) -> f32 {
        let most = self.threat.values().copied().fold(0.0, f32::max);
        let hatred: f32 = if most > 0.0 { contact.crew.iter().map(|id| self.threat.get(id).copied().unwrap_or(0.0) / most).fold(0.0, f32::max) } else { 0.0 };
        let vehicle = if contact.category == Category::Infantry { 0.0 } else { 1.0 };
        (1.0 + 3.0 * hatred + vehicle) * (1.0 + 2.0 * self.share(contact.category))
    }

    /// Where a contact will be in `seconds`, from where he last saw it and how it was moving.
    fn lead(&self, contact: &Contact, seconds: f32) -> [f32; 3] {
        let Some(seen) = self.sightings.get(&contact.id) else { return contact.position };
        if !seen.visible {
            return seen.position;
        }
        let ahead = seen.velocity.map(|v| v * seconds);
        let length = ahead[0].hypot(ahead[2]);
        let scale = if length > MAX_LEAD { MAX_LEAD / length } else { 1.0 };
        [seen.position[0] + ahead[0] * scale, seen.position[1], seen.position[2] + ahead[2] * scale]
    }

    /// Picks the next attack, or none, for him standing at `at`. `contacts` are as `perceive` was
    /// just given them; he stomps on what he sees or has just heard. Without `can_call` (his mouth is
    /// gone) he calls no airstrikes. `noise` gives a random number in 0..1 for each score.
    pub fn decide(&mut self, tick: u32, at: [f32; 3], contacts: &[Contact], can_call: bool, mut noise: impl FnMut() -> f32) -> Decision {
        let mut best = (MIN_SCORE, Decision::Walk);
        let mut consider = |score: f32, decision: Decision, noise: f32| {
            let score = score * (1.0 + NOISE * (2.0 * noise - 1.0));
            if score > best.0 {
                best = (score, decision);
            }
        };
        if tick >= self.stomp_ready {
            let underfoot: Vec<(&Contact, [f32; 3])> = contacts.iter().filter(|c| c.category != Category::Air).filter_map(|c| Some((c, self.now(tick, c)?))).collect();
            for &(_, point) in &underfoot {
                if flat_distance(at, point) > PLAGUE_STOMP_REACH {
                    continue;
                }
                let score: f32 = underfoot.iter().filter(|(_, p)| flat_distance(*p, point) < PLAGUE_STOMP_RADIUS * 0.8).map(|(c, _)| self.weight(c)).sum();
                consider(score * STOMP_PREFERENCE, Decision::Stomp(point), noise());
            }
        }
        if can_call && tick >= self.strike_ready {
            let lead = strike_lead_seconds();
            // Where each contact will be when the rockets come down, as best he can tell.
            let ahead: Vec<(&Contact, [f32; 3], f32)> = contacts
                .iter()
                .filter(|c| c.visible || self.sightings.contains_key(&c.id))
                .map(|c| {
                    let mut weight = self.weight(c);
                    if !c.visible {
                        weight *= HIDDEN_WEIGHT;
                    }
                    if c.category == Category::Air {
                        weight *= AIR_STRIKE_WEIGHT;
                    }
                    (c, self.lead(c, lead), weight)
                })
                .collect();
            for &(_, point, _) in &ahead {
                let distance = flat_distance(at, point);
                // Not on himself: anything that close gets the foot.
                if !(PLAGUE_STOMP_REACH..=PLAGUE_STRIKE_RANGE).contains(&distance) {
                    continue;
                }
                let score: f32 = ahead.iter().filter(|(_, p, _)| flat_distance(*p, point) < PLAGUE_STRIKE_RADIUS).map(|(_, _, w)| w).sum();
                consider(score, Decision::Strike(point), noise());
            }
        }
        match best.1 {
            Decision::Stomp(_) => self.stomp_ready = tick + PLAGUE_STOMP_WINDUP_TICKS + STOMP_COOLDOWN_TICKS,
            Decision::Strike(_) => self.strike_ready = tick + PLAGUE_STRIKE_WINDUP_TICKS + STRIKE_COOLDOWN_TICKS,
            Decision::Walk => {}
        }
        best.1
    }

    /// Pushes the next airstrike back: his call was cut off.
    pub fn delay_strike(&mut self, tick: u32) {
        self.strike_ready = self.strike_ready.max(tick + STRIKE_COOLDOWN_TICKS);
    }
}
