//! Items a player can carry in the hotbar. Mirrored in `client/src/sim/items.ts`.

use crate::consts::TICK_RATE;
use crate::protocol::{WEAPON_GRENADE, WEAPON_REVOLVER, WEAPON_RIFLE, WEAPON_SHOTGUN, WEAPON_SNIPER};

/// Slots in the hotbar. A player picks one with the `slot` field of their input.
pub const HOTBAR_SLOTS: usize = 5;

/// `ItemSlot::0` on the wire and in ground pickups. Zero is an empty slot.
pub const ITEM_NONE: u8 = 0;
pub const ITEM_REVOLVER: u8 = 1;
pub const ITEM_AK5: u8 = 2;
pub const ITEM_GRENADE: u8 = 3;
pub const ITEM_BANDAGE: u8 = 4;
pub const ITEM_SNIPER: u8 = 5;
/// Dropped by a rooster, never scattered. Heals to full.
pub const ITEM_GOLDEN_EGG: u8 = 6;
pub const ITEM_SHOTGUN: u8 = 7;
/// One past the largest item id.
pub const ITEM_COUNT: u8 = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind {
    /// A hitscan weapon fired from the eye.
    Gun,
    /// Thrown, and bursts after a fuse.
    Grenade,
    /// Used over time to restore health.
    Heal,
}

/// How a hitscan round loses damage with distance: all of it up to `start` metres, then a straight
/// line down to `min` (a fraction) at `end`, and `min` beyond.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Falloff {
    pub start: f32,
    pub end: f32,
    pub min: f32,
}

impl Falloff {
    /// Full damage at any distance.
    pub const NONE: Falloff = Falloff { start: 0.0, end: 0.0, min: 1.0 };

    /// The fraction of its damage a round keeps after `dist` metres.
    pub fn scale(&self, dist: f32) -> f32 {
        if dist <= self.start {
            return 1.0;
        }
        let along = if self.end > self.start { ((dist - self.start) / (self.end - self.start)).min(1.0) } else { 1.0 };
        1.0 - (1.0 - self.min) * along
    }

    /// `damage` after `dist` metres: never less than 1, or a far hit would do nothing at all.
    pub fn apply(&self, damage: u8, dist: f32) -> u8 {
        if damage == 0 {
            return 0;
        }
        ((f32::from(damage) * self.scale(dist)).round() as u8).max(1)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct ItemDef {
    pub name: &'static str,
    pub kind: ItemKind,
    /// Held fire keeps shooting (a gun) rather than needing a fresh press.
    pub auto: bool,
    /// Hitscan damage, or health restored by a heal.
    pub damage: u8,
    /// Ticks between uses.
    pub cooldown_ticks: u32,
    /// Rounds in a full magazine; zero for items that do not reload.
    pub mag: u8,
    pub reload_ticks: u32,
    pub range: f32,
    /// Random half-angle of the cone a shot is fired in, radians.
    pub spread: f32,
    /// Fires a round that flies (`PROJECTILE_BULLET`: speed `BULLET_SPEED`, a little drop) and hits when it arrives, instead of a hitscan.
    pub projectile: bool,
    /// Hitscan rays one trigger pull fires, each doing `damage`, each scattered on its own.
    pub pellets: u8,
    /// How `damage` falls off with the distance a ray travelled before it hit.
    pub falloff: Falloff,
    /// Walking speed multiplier while aim is held (`BUTTON_ALT`) with this gun in hand.
    pub aim_speed: f32,
    /// Can be scoped (aim held): the shot then leaves the eye along the look direction, with no spread.
    pub scope: bool,
    /// Most that fit in one slot.
    pub stack: u8,
    /// How it is named in shot and kill events.
    pub weapon: u8,
    /// How likely it is to be picked when pickups are scattered; relative weights.
    pub weight: u32,
}

const REVOLVER: ItemDef = ItemDef {
    name: "Revolver",
    kind: ItemKind::Gun,
    auto: false,
    damage: 35,
    cooldown_ticks: 30,
    mag: 6,
    reload_ticks: 2 * TICK_RATE,
    range: 150.0,
    // A little wild from the hip; aiming tightens it.
    spread: 0.015,
    projectile: true,
    pellets: 1,
    falloff: Falloff { start: 30.0, end: 150.0, min: 0.6 },
    aim_speed: 0.8,
    scope: false,
    stack: 1,
    weapon: WEAPON_REVOLVER,
    weight: 3,
};

const AK5: ItemDef = ItemDef {
    name: "AK5",
    kind: ItemKind::Gun,
    auto: true,
    damage: 12,
    cooldown_ticks: 6,
    mag: 30,
    reload_ticks: 5 * TICK_RATE / 2,
    range: 200.0,
    spread: 0.012,
    projectile: true,
    pellets: 1,
    falloff: Falloff { start: 25.0, end: 200.0, min: 0.5 },
    aim_speed: 0.6,
    scope: false,
    stack: 1,
    weapon: WEAPON_RIFLE,
    weight: 2,
};

const SNIPER: ItemDef = ItemDef {
    name: "Sniper",
    kind: ItemKind::Gun,
    auto: false,
    damage: 90,
    cooldown_ticks: 2 * TICK_RATE,
    mag: 5,
    reload_ticks: 3 * TICK_RATE,
    range: 400.0,
    // Wild from the hip; scoping removes it.
    spread: 0.05,
    projectile: true,
    pellets: 1,
    falloff: Falloff::NONE,
    aim_speed: 0.4,
    scope: true,
    stack: 1,
    weapon: WEAPON_SNIPER,
    weight: 3,
};

const SHOTGUN: ItemDef = ItemDef {
    name: "Shotgun",
    kind: ItemKind::Gun,
    auto: false,
    // Per pellet: ten of them at point blank are more than a full health bar.
    damage: 12,
    cooldown_ticks: TICK_RATE,
    mag: 5,
    reload_ticks: 3 * TICK_RATE,
    range: 40.0,
    // A wide cone from the hip; aiming tightens it.
    spread: 0.06,
    projectile: false,
    pellets: 10,
    falloff: Falloff { start: 8.0, end: 40.0, min: 0.25 },
    aim_speed: 0.7,
    scope: false,
    stack: 1,
    weapon: WEAPON_SHOTGUN,
    weight: 3,
};

const GRENADE: ItemDef = ItemDef {
    name: "Grenade",
    kind: ItemKind::Grenade,
    auto: false,
    damage: 0,
    cooldown_ticks: TICK_RATE,
    mag: 0,
    reload_ticks: 0,
    range: 0.0,
    spread: 0.0,
    projectile: false,
    pellets: 1,
    falloff: Falloff::NONE,
    aim_speed: 1.0,
    scope: false,
    stack: 3,
    weapon: WEAPON_GRENADE,
    weight: 4,
};

const BANDAGE: ItemDef = ItemDef {
    name: "Bandage",
    kind: ItemKind::Heal,
    auto: false,
    damage: 40,
    cooldown_ticks: 0,
    mag: 0,
    reload_ticks: 0,
    range: 0.0,
    spread: 0.0,
    projectile: false,
    pellets: 1,
    falloff: Falloff::NONE,
    aim_speed: 1.0,
    scope: false,
    stack: 5,
    weapon: WEAPON_RIFLE,
    weight: 5,
};

const GOLDEN_EGG: ItemDef = ItemDef {
    name: "Golden egg",
    kind: ItemKind::Heal,
    auto: false,
    // Saturates at `PLAYER_MAX_HP`: a full heal.
    damage: u8::MAX,
    cooldown_ticks: 0,
    mag: 0,
    reload_ticks: 0,
    range: 0.0,
    spread: 0.0,
    projectile: false,
    pellets: 1,
    falloff: Falloff::NONE,
    aim_speed: 1.0,
    scope: false,
    stack: 1,
    weapon: WEAPON_RIFLE,
    // Only roosters drop it.
    weight: 0,
};

/// The definition of `item`; `None` for an empty slot or an unknown id.
pub fn def(item: u8) -> Option<&'static ItemDef> {
    match item {
        ITEM_REVOLVER => Some(&REVOLVER),
        ITEM_AK5 => Some(&AK5),
        ITEM_GRENADE => Some(&GRENADE),
        ITEM_BANDAGE => Some(&BANDAGE),
        ITEM_SNIPER => Some(&SNIPER),
        ITEM_SHOTGUN => Some(&SHOTGUN),
        ITEM_GOLDEN_EGG => Some(&GOLDEN_EGG),
        _ => None,
    }
}

/// The gun a shot event of `weapon` came from, or `ITEM_NONE`.
pub fn gun_for_weapon(weapon: u8) -> u8 {
    (1..ITEM_COUNT).find(|&i| def(i).is_some_and(|d| d.kind == ItemKind::Gun && d.weapon == weapon)).unwrap_or(ITEM_NONE)
}

/// How much slower the player of `item` (in hand, or 0 for none) walks with `buttons`: only a gun
/// slows its holder, and only while aim is held. Applied to walk, run and crouch speed alike.
pub fn aim_speed_scale(item: u8, buttons: u16) -> f32 {
    match def(item) {
        Some(d) if d.kind == ItemKind::Gun && buttons & crate::consts::BUTTON_ALT != 0 => d.aim_speed,
        _ => 1.0,
    }
}

/// Ticks a bandage takes to apply.
pub const BANDAGE_TICKS: u32 = 2 * TICK_RATE;

/// Grenades: thrown along the look direction, they fall, bounce, and burst when the fuse runs out.
pub const GRENADE_SPEED: f32 = 16.0;
pub const GRENADE_FUSE_TICKS: u32 = 3 * TICK_RATE;
pub const GRENADE_BLAST_RADIUS: f32 = 5.0;
pub const GRENADE_DAMAGE: f32 = 100.0;
pub const GRENADE_KNOCK: f32 = 12.0;
/// How much of its speed a grenade keeps when it bounces.
pub const GRENADE_BOUNCE: f32 = 0.4;
/// A throw is aimed this far above the look direction, radians, so it arcs, and starts this far
/// in front of the eye, metres.
pub const GRENADE_LOFT: f32 = 0.15;
pub const GRENADE_MUZZLE: f32 = 0.6;
/// Collision radius of a grenade, metres.
pub const GRENADE_RADIUS: f32 = 0.1;
/// Grenades in flight at once, which bounds the projectile list.
pub const MAX_GRENADES: usize = 16;

/// Ground pickups. A player in `PICKUP_RADIUS` of one who presses use takes it.
pub const PICKUP_RADIUS: f32 = 2.0;
/// Pickups in the world, and how long a taken one stays away.
pub const PICKUP_COUNT: usize = 40;
pub const PICKUP_RESPAWN_TICKS: u32 = 30 * TICK_RATE;
/// Most pickups one snapshot lists: they are static, so the rest arrive as the player nears them.
pub const MAX_PICKUPS_PER_SNAPSHOT: usize = 24;

#[cfg(test)]
mod tests {
    use super::*;

    /// The item table and the scalar constants the client mirrors, as JSON. `fixtures/parity/
    /// gameplay.json` holds the committed copy, which `client/src/__tests__/parity.spec.ts` checks the
    /// TypeScript tables against. After an intended change, regenerate it with `UPDATE_PARITY=1 cargo
    /// test -p shared parity` and update `client/src/sim/items.ts` to match.
    fn parity_json() -> String {
        let items: Vec<String> = (1..ITEM_COUNT)
            .map(|id| {
                let d = def(id).expect("every id below ITEM_COUNT is defined");
                let kind = match d.kind {
                    ItemKind::Gun => "gun",
                    ItemKind::Grenade => "grenade",
                    ItemKind::Heal => "heal",
                };
                format!(
                    "    \"{id}\": {{\"name\": \"{}\", \"kind\": \"{kind}\", \"auto\": {}, \"damage\": {}, \"cooldownTicks\": {}, \"mag\": {}, \"reloadTicks\": {}, \"range\": {}, \"aimSpeed\": {}, \"scope\": {}, \"stack\": {}, \"weapon\": {}, \"projectile\": {}, \"pellets\": {}, \"falloffStart\": {}, \"falloffEnd\": {}, \"falloffMin\": {}}}",
                    d.name, d.auto, d.damage, d.cooldown_ticks, d.mag, d.reload_ticks, d.range, d.aim_speed, d.scope, d.stack, d.weapon, d.projectile, d.pellets, d.falloff.start, d.falloff.end, d.falloff.min
                )
            })
            .collect();
        let vehicles: Vec<String> = crate::vehicles::VEHICLES
            .iter()
            .map(|d| {
                let [hx, hy, hz] = d.half_extents;
                format!(
                    "    \"{}\": {{\"name\": \"{}\", \"hp\": {}, \"seats\": {}, \"halfExtents\": [{hx}, {hy}, {hz}], \"gun\": {}, \"arms\": {}, \"carWheels\": {}, \"trackHalf\": {}, \"axleZ\": {}}}",
                    d.kind, d.name, d.hp, d.seats, d.gun, d.arms, d.car_wheels, d.track_half, d.axle_z
                )
            })
            .collect();
        let tree_kinds: Vec<String> = crate::world::TREE_KINDS
            .iter()
            .map(|d| {
                format!(
                    "    \"{}\": {{\"name\": \"{}\", \"radius\": {}, \"refHeight\": {}, \"radiusPctOfCrown\": {}, \"maxRadius\": {}, \"colliderHeightPct\": {}, \"destructible\": {}}}",
                    d.kind, d.name, d.radius, d.ref_height, d.radius_pct_of_crown, d.max_radius, d.collider_height_pct, d.destructible
                )
            })
            .collect();
        // Each value is printed from its own type, so an f32 0.15 stays 0.15 rather than 0.15000000596.
        let consts = [
            ("TICK_RATE", TICK_RATE.to_string()),
            ("HOTBAR_SLOTS", HOTBAR_SLOTS.to_string()),
            ("BANDAGE_TICKS", BANDAGE_TICKS.to_string()),
            ("GRENADE_SPEED", GRENADE_SPEED.to_string()),
            ("GRENADE_FUSE_TICKS", GRENADE_FUSE_TICKS.to_string()),
            ("GRENADE_BLAST_RADIUS", GRENADE_BLAST_RADIUS.to_string()),
            ("GRENADE_LOFT", GRENADE_LOFT.to_string()),
            ("GRENADE_MUZZLE", GRENADE_MUZZLE.to_string()),
            ("GRENADE_RADIUS", GRENADE_RADIUS.to_string()),
            ("GRENADE_BOUNCE", GRENADE_BOUNCE.to_string()),
            ("PICKUP_RADIUS", PICKUP_RADIUS.to_string()),
        ];
        let lines: Vec<String> = consts.iter().map(|(name, value)| format!("    \"{name}\": {value}")).collect();
        format!(
            "{{\n  \"items\": {{\n{}\n  }},\n  \"vehicles\": {{\n{}\n  }},\n  \"treeKinds\": {{\n{}\n  }},\n  \"consts\": {{\n{}\n  }}\n}}\n",
            items.join(",\n"),
            vehicles.join(",\n"),
            tree_kinds.join(",\n"),
            lines.join(",\n")
        )
    }

    #[test]
    fn parity_fixture_matches_the_tables() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/parity/gameplay.json");
        let actual = parity_json();
        if std::env::var_os("UPDATE_PARITY").is_some() {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, &actual).unwrap();
        }
        let committed = std::fs::read_to_string(&path).expect("fixtures/parity/gameplay.json; regenerate with UPDATE_PARITY=1");
        assert_eq!(committed, actual, "gameplay tables changed: regenerate the fixture and update client/src/sim/items.ts");
    }

    #[test]
    fn only_a_gun_with_aim_held_slows_its_holder() {
        use crate::consts::BUTTON_ALT;
        assert_eq!(aim_speed_scale(ITEM_AK5, 0), 1.0);
        assert_eq!(aim_speed_scale(ITEM_AK5, BUTTON_ALT), AK5.aim_speed);
        assert!(aim_speed_scale(ITEM_SNIPER, BUTTON_ALT) < aim_speed_scale(ITEM_REVOLVER, BUTTON_ALT));
        assert_eq!(aim_speed_scale(ITEM_GRENADE, BUTTON_ALT), 1.0);
        assert_eq!(aim_speed_scale(ITEM_NONE, BUTTON_ALT), 1.0);
    }

    #[test]
    fn damage_falls_off_with_distance_but_never_to_nothing() {
        let f = Falloff { start: 10.0, end: 50.0, min: 0.5 };
        assert_eq!(f.scale(0.0), 1.0);
        assert_eq!(f.scale(10.0), 1.0);
        assert!((f.scale(30.0) - 0.75).abs() < 1e-6);
        assert_eq!(f.scale(50.0), 0.5);
        assert_eq!(f.scale(500.0), 0.5);
        assert_eq!(Falloff::NONE.scale(1000.0), 1.0);
        assert_eq!(f.apply(100, 50.0), 50);
        assert_eq!(f.apply(1, 50.0), 1);
        assert_eq!(f.apply(0, 50.0), 0);
        // Ten close-range pellets are lethal; ten from beyond the edge of the range are not.
        let gun = def(ITEM_SHOTGUN).unwrap();
        let close = u32::from(gun.damage) * u32::from(gun.pellets);
        assert!(close >= u32::from(crate::consts::PLAYER_MAX_HP));
        let far = u32::from(gun.falloff.apply(gun.damage, gun.range)) * u32::from(gun.pellets);
        assert!(far < u32::from(crate::consts::PLAYER_MAX_HP) / 2);
    }

    #[test]
    fn the_golden_egg_heals_to_full() {
        let egg = def(ITEM_GOLDEN_EGG).unwrap();
        assert_eq!(egg.kind, ItemKind::Heal);
        assert!(egg.damage >= crate::consts::PLAYER_MAX_HP);
    }

    #[test]
    fn every_item_id_has_a_definition() {
        assert!(def(ITEM_NONE).is_none());
        for item in 1..ITEM_COUNT {
            let d = def(item).unwrap_or_else(|| panic!("item {item} has no definition"));
            assert!(d.stack >= 1);
            assert!(d.weight > 0 || item == ITEM_GOLDEN_EGG);
            assert!(d.aim_speed > 0.0 && d.aim_speed <= 1.0);
        }
        assert!(def(ITEM_COUNT).is_none());
    }
}
