//! Protocol v2's entity state: what each replicated thing is, as components (design.md §2.1).
//!
//! An entity on the wire is the components it has, not a row of its family. Every component is listed
//! once, in the `replicated!` invocation below: its wire id, its fields, and whether it is sent as a
//! difference (`diff`) or whole (`set`). Everything else (the structs, the wire form, diffing,
//! applying, the encoding and the schema the client decodes with) is made from that list, so a new
//! component is one entry there and nothing else on either side.
//!
//! An update is one MessagePack array, `[id, set, (cleared), fields…]`:
//!
//! - `set` is a bit mask of the components whose fields follow, in wire-id order, each component's
//!   fields one after another (no array of their own).
//! - Bit 31 of `set` (`CLEARED`) says a `cleared` mask follows: the components the entity no longer
//!   has, such as a repaired hull's `damage` or a turret shot off.
//! - A `diff` component is sent as the difference from the receiver's baseline when that baseline
//!   has it, and whole when it does not: its fields widened to `i32` either way. Both sides know which,
//!   since both hold the baseline.
//!
//! An update for an id not in the baseline brings the whole entity: everything it has is set.

use crate::items::HOTBAR_SLOTS;
use crate::protocol::ItemSlot;
use serde::de::{self, SeqAccess, Visitor};
use std::borrow::Borrow;
use serde::ser::SerializeSeq;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Bit 31 of an update's `set` mask: a `cleared` mask follows it.
pub const CLEARED: u32 = 1 << 31;

/// What every component can do on the wire. Made by `replicated!`; nothing implements it by hand.
pub trait Component: Clone + PartialEq {
    /// Its bit in an update's masks, never reused for another component.
    const ID: u8;
    /// Sent as a difference from the baseline's, rather than whole.
    const DIFF: bool;
    /// Sent only to the receiver who owns the entity (`Replicated::public` leaves it out for anyone
    /// else): the player's own entity, or the vehicle they drive.
    const OWNER: bool;
    /// Its fields in wire order.
    const FIELDS: &'static [Field];
    /// How it goes on the wire: itself for a `set` component, its fields as `i32` for a `diff` one.
    type Wire: Clone + PartialEq + std::fmt::Debug;
    /// The whole value, for a receiver whose baseline does not have it.
    fn full(&self) -> Self::Wire;
    /// The change from `base`, for a receiver whose baseline has `base`.
    fn change(&self, base: &Self) -> Self::Wire;
    /// The value `wire` gives, on top of the baseline's `base` (whose presence says which form it is).
    fn rebuild(base: Option<&Self>, wire: &Self::Wire) -> Self;
    fn write<S: SerializeSeq>(wire: &Self::Wire, seq: &mut S) -> Result<(), S::Error>;
    fn read<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<Self::Wire, A::Error>;
}

/// A component's field: its name, its Rust type, and its unit (`UNITS`), or `""` for a count, a set of
/// bits or an id, which have none.
pub type Field = (&'static str, &'static str, &'static str);

/// What a field's integer counts, and so how the client turns it into its own units and whether it
/// blends two snapshots' values or takes the nearer one's:
///
/// - `cm`: centimetres (or cm/s), metres on the client; blended.
/// - `mrad`: milliradians, an angle; blended the short way round.
/// - `quat`: part of a unit quaternion scaled by 32767, `w >= 0` left out; blended as a rotation.
/// - `axis`: -127..=127 for -1..=1; blended.
/// - `byte`: 0..=255 for 0..=1, a gauge; the nearer snapshot's.
/// - `dm`: tenths of a metre; the nearer snapshot's.
pub const UNITS: &[&str] = &["cm", "mrad", "quat", "axis", "byte", "dm"];

/// One component as `schema` lists it: its wire id, its field in `Replicated`, its struct, whether it
/// is sent as a difference and only to its owner, and its fields.
#[derive(Clone, Copy, Debug)]
pub struct Schema {
    pub id: u8,
    pub name: &'static str,
    pub ty: &'static str,
    pub diff: bool,
    pub owner: bool,
    pub fields: &'static [Field],
}

fn next<'de, T: Deserialize<'de>, A: SeqAccess<'de>>(seq: &mut A) -> Result<T, A::Error> {
    seq.next_element()?.ok_or_else(|| de::Error::custom("an update cut short"))
}

/// Whether a component's entry in `replicated!` is marked `[owner]`.
macro_rules! owner {
    () => {
        false
    };
    (owner) => {
        true
    };
}

macro_rules! count {
    () => { 0 };
    ($head:ident $($rest:ident)*) => { 1 + count!($($rest)*) };
}

/// One component's struct and its `Component` impl.
macro_rules! component {
    (diff $id:literal $owner:tt $(#[$meta:meta])* $Name:ident { $( $(#[$fmeta:meta])* $f:ident : $t:ty $(as $unit:ident)? ),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
        pub struct $Name { $( $(#[$fmeta])* pub $f: $t, )* }

        impl Component for $Name {
            const ID: u8 = $id;
            const DIFF: bool = true;
            const OWNER: bool = $owner;
            const FIELDS: &'static [Field] = &[$( (stringify!($f), stringify!($t), stringify!($($unit)?)) ),*];
            type Wire = [i32; count!($($f)*)];
            fn full(&self) -> Self::Wire {
                [$( self.$f as i32 ),*]
            }
            fn change(&self, base: &Self) -> Self::Wire {
                [$( (self.$f as i32).wrapping_sub(base.$f as i32) ),*]
            }
            fn rebuild(base: Option<&Self>, wire: &Self::Wire) -> Self {
                let mut parts = wire.iter();
                $( let $f = base.map_or(0, |base| base.$f as i32).wrapping_add(*parts.next().expect("one per field")) as $t; )*
                $Name { $( $f ),* }
            }
            fn write<S: SerializeSeq>(wire: &Self::Wire, seq: &mut S) -> Result<(), S::Error> {
                wire.iter().try_for_each(|part| seq.serialize_element(part))
            }
            fn read<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<Self::Wire, A::Error> {
                let mut wire = [0; count!($($f)*)];
                for part in &mut wire {
                    *part = next(seq)?;
                }
                Ok(wire)
            }
        }

        #[cfg(test)]
        impl $Name {
            fn json(&self) -> String {
                let fields: Vec<String> = vec![$( format!("\"{}\": {}", stringify!($f), json::Json::json(&self.$f)) ),*];
                format!("{{{}}}", fields.join(", "))
            }
        }
    };
    (set $id:literal $owner:tt $(#[$meta:meta])* $Name:ident { $( $(#[$fmeta:meta])* $f:ident : $t:ty $(as $unit:ident)? ),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Clone, Debug, Default, PartialEq, Eq)]
        pub struct $Name { $( $(#[$fmeta])* pub $f: $t, )* }

        impl Component for $Name {
            const ID: u8 = $id;
            const DIFF: bool = false;
            const OWNER: bool = $owner;
            const FIELDS: &'static [Field] = &[$( (stringify!($f), stringify!($t), stringify!($($unit)?)) ),*];
            type Wire = Self;
            fn full(&self) -> Self {
                self.clone()
            }
            fn change(&self, _: &Self) -> Self {
                self.clone()
            }
            fn rebuild(_: Option<&Self>, wire: &Self) -> Self {
                wire.clone()
            }
            fn write<S: SerializeSeq>(wire: &Self, seq: &mut S) -> Result<(), S::Error> {
                $( seq.serialize_element(&wire.$f)?; )*
                Ok(())
            }
            fn read<'de, A: SeqAccess<'de>>(seq: &mut A) -> Result<Self, A::Error> {
                Ok($Name { $( $f: next(seq)? ),* })
            }
        }

        #[cfg(test)]
        impl $Name {
            fn json(&self) -> String {
                let fields: Vec<String> = vec![$( format!("\"{}\": {}", stringify!($f), json::Json::json(&self.$f)) ),*];
                format!("{{{}}}", fields.join(", "))
            }
        }
    };
}

/// Every component, and the `Replicated` and `Update` structs that hold them. Listed in wire-id
/// order, which is the order their fields go on the wire.
macro_rules! replicated {
    ($( $mode:ident $id:literal $field:ident $([$owner:ident])? $(#[$meta:meta])* $Name:ident { $($body:tt)* } )*) => {
        $( component!($mode $id { owner!($($owner)?) } $(#[$meta])* $Name { $($body)* }); )*

        /// An entity as it is replicated: the components it has.
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Replicated {
            $( pub $field: Option<$Name>, )*
        }

        /// What one snapshot says about one entity: the components set (each in its wire form) and
        /// the ones cleared since the baseline.
        #[derive(Clone, Debug, Default, PartialEq)]
        pub struct Update {
            pub id: u32,
            /// Bits of the components it no longer has.
            pub cleared: u32,
            $( pub $field: Option<<$Name as Component>::Wire>, )*
        }

        /// Bits of the `[owner]` components.
        pub const OWNER_MASK: u32 = 0 $( | if <$Name as Component>::OWNER { 1 << <$Name as Component>::ID } else { 0 } )*;

        impl Replicated {
            /// Bits of the components it has.
            pub fn mask(&self) -> u32 {
                0 $( | if self.$field.is_some() { 1 << <$Name as Component>::ID } else { 0 } )*
            }

            /// What anyone but its owner is told of it: everything but the `[owner]` components.
            pub fn public(&self) -> Replicated {
                Replicated { $( $field: if <$Name as Component>::OWNER { None } else { self.$field.clone() }, )* }
            }

            /// What entity `id` being `self` says to a receiver whose baseline holds it as `base` (or
            /// not at all). `None` when the baseline holds it and nothing changed.
            pub fn update(&self, id: u32, base: Option<&Replicated>) -> Option<Update> {
                let mut update = Update { id, ..Default::default() };
                $(
                    match (base.and_then(|base| base.$field.as_ref()), self.$field.as_ref()) {
                        (Some(was), Some(now)) if was != now => update.$field = Some(now.change(was)),
                        (None, Some(now)) => update.$field = Some(now.full()),
                        (Some(_), None) => update.cleared |= 1 << <$Name as Component>::ID,
                        _ => {}
                    }
                )*
                (base.is_none() || update.set() != 0).then_some(update)
            }

            /// The entity `update` makes of the baseline's `base` (or of nothing).
            pub fn apply(base: Option<&Replicated>, update: &Update) -> Replicated {
                let mut now = base.cloned().unwrap_or_default();
                $(
                    if update.cleared & (1 << <$Name as Component>::ID) != 0 {
                        now.$field = None;
                    }
                    if let Some(wire) = &update.$field {
                        now.$field = Some(<$Name as Component>::rebuild(base.and_then(|base| base.$field.as_ref()), wire));
                    }
                )*
                now
            }
        }

        impl Update {
            /// Bits of the components it sets, with `CLEARED` when any are cleared.
            pub fn set(&self) -> u32 {
                let set = 0 $( | if self.$field.is_some() { 1 << <$Name as Component>::ID } else { 0 } )*;
                if self.cleared != 0 { set | CLEARED } else { set }
            }
        }

        impl Serialize for Update {
            fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
                let set = self.set();
                let len = 2 + usize::from(self.cleared != 0) $( + if self.$field.is_some() { <$Name as Component>::FIELDS.len() } else { 0 } )*;
                let mut seq = serializer.serialize_seq(Some(len))?;
                seq.serialize_element(&self.id)?;
                seq.serialize_element(&set)?;
                if self.cleared != 0 {
                    seq.serialize_element(&self.cleared)?;
                }
                $(
                    if let Some(wire) = &self.$field {
                        <$Name as Component>::write(wire, &mut seq)?;
                    }
                )*
                seq.end()
            }
        }

        impl<'de> Deserialize<'de> for Update {
            fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
                struct Fields;
                impl<'de> Visitor<'de> for Fields {
                    type Value = Update;
                    fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                        formatter.write_str("an entity update: [id, set, (cleared), fields…]")
                    }
                    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Update, A::Error> {
                        let id = next(&mut seq)?;
                        let set: u32 = next(&mut seq)?;
                        let cleared = if set & CLEARED != 0 { next(&mut seq)? } else { 0 };
                        let has = |id: u8| set & (1 << id) != 0;
                        Ok(Update {
                            id,
                            cleared,
                            $( $field: if has(<$Name as Component>::ID) { Some(<$Name as Component>::read(&mut seq)?) } else { None }, )*
                        })
                    }
                }
                deserializer.deserialize_seq(Fields)
            }
        }

        #[cfg(test)]
        impl Replicated {
            /// The entity as JSON: each component it has by name, as an object of its fields.
            fn json(&self) -> String {
                let mut parts: Vec<String> = Vec::new();
                $( if let Some(component) = &self.$field { parts.push(format!("\"{}\": {}", stringify!($field), component.json())); } )*
                format!("{{{}}}", parts.join(", "))
            }
        }

        /// Each component as the client decodes it.
        pub fn schema() -> Vec<Schema> {
            vec![$( Schema { id: <$Name as Component>::ID, name: stringify!($field), ty: stringify!($Name), diff: <$Name as Component>::DIFF, owner: <$Name as Component>::OWNER, fields: <$Name as Component>::FIELDS } ),*]
        }
    };
}

replicated! {
    diff 0 place
    /// Where it is, centimetres: a soldier's capsule centre, a vehicle's chassis centre.
    Place { x: i32 as cm, y: i32 as cm, z: i32 as cm }

    diff 1 heading
    /// Its facing, milliradians, 0 facing -Z.
    Heading { yaw: i16 as mrad }

    diff 2 motion
    /// What a predicted soldier resumes from: vertical speed, and the slide a car's knock left
    /// them in, cm/s.
    Motion { vy: i16 as cm, knock_x: i16 as cm, knock_z: i16 as cm }

    set 3 flags
    /// `FLAG_*` bits and the `STANCE_*`.
    Flags { flags: u8, stance: u8 }

    diff 4 orientation
    /// A vehicle's chassis as the x, y, z of a unit quaternion scaled by 32767, `w >= 0`.
    Orientation { qx: i16 as quat, qy: i16 as quat, qz: i16 as quat }

    diff 5 steer
    /// Front wheels as a fraction of full lock, -127..=127, + is right.
    Steer { steer: i8 as axis }

    set 6 turret
    /// A gun on a mount, as it is drawn: the turret's yaw (+ turns left) and the barrel's elevation (+ is
    /// up), both in the hull's frame; the shots fired, modulo 256, so a client sees a new one as a change;
    /// a field mortar's range dial; and `TURRET_*` flags.
    Turret { yaw: i16 as mrad, pitch: i16 as mrad, shots: u8, range: u16 as dm, flags: u8 }

    set 7 limbs
    /// The limbs a soldier has lost (`LIMB_*`, low four bits) and the stumps that still bleed (the same
    /// bits above `LIMB_BLEED_SHIFT`). Absent while whole. Ids 7 and 10 were the vehicle's `crew` and
    /// `seats`.
    Limbs { lost: u8 }

    set 8 thrust
    /// What is left of a Roadster's air, a hen's wings' stamina or a drone's battery, 0..=255 of full,
    /// and the `THRUST_*` flags: the nozzles firing, a hen's glide or lift, a drone still linked.
    Thrust { air: u8 as byte, flags: u8 }

    set 9 arms
    /// A plane's pilot weapons and engine, as drawn and shown on the crew's gauges: the forward guns'
    /// rounds modulo 256 (the left gun fires the even ones), the heat of both sets of guns (they jam at
    /// 255), the bombs on the rack, and `ARMS_*` flags. A hen sends its clutch of eggs as `bombs`.
    Arms { shots: u8, heat: u8 as byte, rear_heat: u8 as byte, bombs: u8, flags: u8 }

    set 11 health
    /// A vehicle's health; 0 is a burning wreck.
    Health { hp: u8 }

    set 12 damage
    /// A hurt hull and wheels, each 0..=255: front, rear, left, right, then the wheels front left, front
    /// right, rear left, rear right (`DAMAGE_*`). A wheel at `WHEEL_GONE` has come off; a tank's slot 4 at
    /// `TURRET_GONE` means its turret has been shot off. Then the parts' states and the fire
    /// (`DAMAGE_PARTS`, `DAMAGE_FIRE`). Absent while unhurt.
    Damage { parts: [u8; 11] }

    diff 13 velocity
    /// A projectile's velocity, cm/s.
    Velocity { vx: i16 as cm, vy: i16 as cm, vz: i16 as cm }

    set 14 kind
    /// `KIND_*`: which model a soldier, a creature or a vehicle is drawn as.
    Kind { kind: u8 }

    set 15 projectile
    /// `PROJECTILE_*`: what is in flight.
    Projectile { kind: u8 }

    set 16 pickup
    /// `ITEM_*`: what lies on the ground here.
    Pickup { item: u8 }

    set 17 vitals [owner]
    /// A player's own torso health (0 is dead), how far their reload or bandage has got, 0..=255 of done,
    /// or 0 when neither is going on, and their stamina on foot as their prediction steps it: the points
    /// spent (0 is rested, `STAMINA_FULL` is out) and whether they are winded, 0 or 1.
    /// `parts` is what is left of the head's health and each limb's (`LIMB_*` order), 0 for a limb lost.
    /// `down` is the ticks of knockdown left (`Character::down`), for their prediction.
    Vitals { hp: u8, progress: u8 as byte, spent: u16, winded: u8, parts: [u8; 5], down: u8 }

    set 18 hotbar [owner]
    /// A player's own hotbar.
    Hotbar { slots: [ItemSlot; HOTBAR_SLOTS] }

    set 19 drive [owner]
    /// The exact motion of a vehicle, which its pose does not carry, for its driver's prediction:
    /// velocity, cm/s; angular velocity about the world axes, mrad/s; the front wheels' angle, mrad,
    /// + is right; and the yaw rate a tank has ramped to, mrad/s, + is clockwise.
    Drive { vx: i16 as cm, vy: i16 as cm, vz: i16 as cm, wx: i16 as mrad, wy: i16 as mrad, wz: i16 as mrad, steer: i16 as mrad, spin: i16 as mrad }

    set 20 seated
    /// A player sitting in seat `seat` of vehicle `vehicle` (0 drives). They have no `place` of their
    /// own unless their body stays behind (a drone's pilot): they are where the kind's seat puts them.
    /// Id 10 was the vehicle's `seats`, which this replaced.
    Seated { vehicle: u32, seat: u8 }

    diff 21 look
    /// How far a rider whose crew show looks up (+) or down, milliradians; their `heading` is where
    /// they look about, in the world.
    Look { pitch: i16 as mrad }

    set 22 carry
    /// What a soldier carries on its back, `CARRY_*`: the backpack (BACKPACK.md). Absent when nothing.
    Carry { what: u8 }

    set 23 struck
    /// The last hit a soldier took this life, so everyone sees it flinch: the hits so far, modulo 256 (a
    /// new one is a change), the part struck (`PART_*`, a limb by its `LIMB_*` bit), how much it lost,
    /// the bearing it came from, in the world, and where on the body it struck, from the point the
    /// soldier is drawn from (`Place`), all zero when not known (a blast). Absent until the first hit
    /// of a life.
    Struck { count: u8, part: u8, amount: u8, yaw: i16 as mrad, x: i16 as cm, y: i16 as cm, z: i16 as cm }
}

/// The structs a component field can hold a list of, with their fields in wire order and each one's
/// unit (`UNITS`): they go on the wire as arrays, which the client turns back into objects.
pub const STRUCTS: &[(&str, &[(&str, &str)])] = &[("ItemSlot", &[("item", ""), ("count", ""), ("ammo", "")])];

/// JSON for the client's fixtures.
#[cfg(test)]
mod json {
    pub trait Json {
        fn json(&self) -> String;
    }
    macro_rules! number {
        ($($t:ty),*) => { $( impl Json for $t { fn json(&self) -> String { self.to_string() } } )* };
    }
    number!(i8, i16, i32, u8, u16, u32);
    impl<T: Json> Json for Vec<T> {
        fn json(&self) -> String {
            format!("[{}]", self.iter().map(Json::json).collect::<Vec<_>>().join(", "))
        }
    }
    impl<T: Json, const N: usize> Json for [T; N] {
        fn json(&self) -> String {
            format!("[{}]", self.iter().map(Json::json).collect::<Vec<_>>().join(", "))
        }
    }
    impl Json for crate::protocol::ItemSlot {
        fn json(&self) -> String {
            format!("{{\"item\": {}, \"count\": {}, \"ammo\": {}}}", self.item, self.count, self.ammo)
        }
    }
}

/// What a receiver holds: every entity it has been told of, by id.
pub type Scene = Vec<(u32, Replicated)>;

/// What turns `base` into `now` (both ordered by id): an update for every entity that is new or
/// changed, in id order, and the ids gone. Either may hold its entities shared (`Arc<Replicated>`), as
/// the server's do.
pub fn diff<B: Borrow<Replicated>, N: Borrow<Replicated>>(base: &[(u32, B)], now: &[(u32, N)]) -> (Vec<Update>, Vec<u32>) {
    let mut updates = Vec::new();
    let mut removed = Vec::new();
    let mut held = base.iter().peekable();
    for (id, entity) in now {
        while let Some((gone, _)) = held.next_if(|(was, _)| was < id) {
            removed.push(*gone);
        }
        let was = held.next_if(|(was, _)| was == id).map(|(_, was)| was.borrow());
        updates.extend(entity.borrow().update(*id, was));
    }
    removed.extend(held.map(|(gone, _)| *gone));
    (updates, removed)
}

/// The scene `updates` and `removed` make of `base`. An update for an id that is neither held nor
/// brings the entity whole still adds it, as what it sets.
pub fn apply(base: &[(u32, Replicated)], updates: &[Update], removed: &[u32]) -> Scene {
    let mut scene: Scene = base.iter().filter(|(id, _)| !removed.contains(id)).cloned().collect();
    for update in updates {
        match scene.binary_search_by_key(&update.id, |(id, _)| *id) {
            Ok(at) => scene[at].1 = Replicated::apply(Some(&scene[at].1), update),
            Err(at) => scene.insert(at, (update.id, Replicated::apply(None, update))),
        }
    }
    scene
}

#[cfg(test)]
pub(crate) mod tests;
