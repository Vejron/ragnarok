//! Game mechanics are small modules that hook into the game loop. Adding a feature (vehicles,
//! pickups, game modes) means adding a `Mechanic`, not editing the loop.

mod armor;
mod backpack;
mod bots;
mod breath;
mod building;
mod carry;
mod carwash;
mod civilians;
mod buses;
mod combat;
mod deliveries;
mod drones;
mod emergency;
mod fuel;
mod gates;
mod ghosts;
mod golf;
mod gunnery;
mod life;
mod lifts;
mod limbs;
mod mend;
mod movement;
mod npcs;
mod pickups;
mod projectiles;
mod scores;
mod spawn;
mod steering;
mod structure;
mod submarines;
mod titans;
mod traffic;
pub(crate) mod vehicles;
mod wildlife;
mod zombies;

pub use breath::Breath;
pub use carwash::Carwash;
pub use civilians::{CIVILIAN_SLANT, Civilian, Civilians, is_noncombatant};
pub use buses::{Bus, BusCall, BusDriver, Buses};
pub use bots::{Bots, DRIVER, GUNNER, nearest_waypoint, road_route};
pub use combat::{Combat, bearing, position_of};
#[cfg(test)]
pub use combat::{cook_off, damage_object, damage_vehicle};
#[cfg(test)]
pub(crate) use combat::CARVE_REACH;
pub(crate) use backpack::{carries, lying as backpack_lying};
pub use armor::ArmorHit;
pub use backpack::{Backpack, BackpackState};
pub(crate) use deliveries::{act as phone, carries_parcel, lying as parcels_lying};
pub use deliveries::{Deliveries, DeliveryState};
pub use drones::Drones;
pub use gates::Gates;
pub use ghosts::{Ghost, Ghosts, shown as ghost_shown};
pub use golf::Golf;
pub use gunnery::Gunnery;
pub use life::{Life, record_trails};
pub use lifts::Lifts;
pub use mend::Mend;
pub use limbs::{Struck, Wounds, lost as limbs_lost};
pub use movement::Movement;
pub use npcs::{Npcs, calling};
pub use pickups::Pickups;
pub use scores::Scores;
pub use spawn::{Spawn, usable as spawn_usable};
pub use structure::Structure;
pub use submarines::{Sub, Submarines};
pub use titans::{Head as TitanHead, Titans, held_in as held_in_fist};
pub use emergency::{Emergency, EmergencyCall, MEDIC_KIND, Medic, OFFICER_KIND, Officer, TEAM_SERVICES, officer_loadout, wanted_byte};
pub use traffic::{Driver, Fleet, Traffic, TrafficCall, seat_crew};
pub use vehicles::{Vehicles, add_tank, remove_vehicle, seat, step_byte};
pub(crate) use vehicles::reclaim_stranded;
pub(crate) use fuel::byte as fuel_byte;
pub use wildlife::{Wildlife, fell, grazing};
pub use zombies::{Zombie, Zombies, clawing, is_zombie, rising, start_rising};
pub(crate) use bots::darkness;
pub(crate) use life::{Harm, damage_entity, open_ground};

use crate::game::World;

/// Whether player `id` is a person: not a bot, a zombie or a civilian (a bus driver, an officer and
/// a medic among them).
pub(crate) fn human(world: &World, id: u32) -> bool {
    !world.entities.has::<crate::game::Bot>(id) && !is_noncombatant(world, id) && !is_zombie(world, id)
}

pub trait Mechanic: Send {
    /// Its name, for timings and logs.
    fn name(&self) -> &'static str {
        let path = std::any::type_name::<Self>();
        path.rsplit("::").next().unwrap_or(path)
    }
    /// The room was just created, before any player joined.
    fn on_start(&mut self, _world: &mut World) {}
    /// A player entity was just created (at the origin, with no input yet).
    fn on_join(&mut self, _world: &mut World, _id: u32) {}
    /// A player entity is about to be removed.
    fn on_leave(&mut self, _world: &mut World, _id: u32) {}
    /// One fixed simulation tick. Each player's input for this tick is already in `Player::input`.
    fn on_tick(&mut self, _world: &mut World, _dt: f32) {}
    /// The reactions it needs run the moment something is hurt or dies (see `crate::events::Hooks`).
    fn hooks(&self, _hooks: &mut crate::events::Hooks) {}
    /// After every mechanic's tick and the scenario's: what this tick's events say happened
    /// (`World::happened`), for a mechanic that answers it, before the players are told.
    fn react(&mut self, _world: &mut World) {}
    /// Itself, for a test that looks inside it (`Game::mechanic`).
    #[cfg(test)]
    fn inside(&self) -> Option<&dyn std::any::Any> {
        None
    }
    /// Itself, for a test that reaches into it (`Game::with_mechanic`).
    #[cfg(test)]
    fn inside_mut(&mut self) -> Option<&mut dyn std::any::Any> {
        None
    }
}

/// Registers the components mechanics own with the room's ECS world (see `entities`).
pub fn register_components(ecs: &mut bevy_ecs::world::World) {
    ecs.register_component::<npcs::Wander>();
    ecs.register_component::<wildlife::Wild>();
    ecs.register_component::<wildlife::Fell>();
    ecs.register_component::<pickups::Pickup>();
    ecs.register_component::<crate::game::Vehicle>();
    ecs.register_component::<crate::game::Mounts>();
    ecs.register_component::<crate::game::Drone>();
    ecs.register_component::<crate::game::Wreck>();
    ecs.register_component::<crate::game::Doomed>();
    ecs.register_component::<crate::game::Burning>();
    ecs.register_component::<crate::game::Seats>();
    ecs.register_component::<crate::game::Autopilot>();
    ecs.register_component::<traffic::TrafficCar>();
    ecs.register_component::<traffic::Abandoned>();
    ecs.register_component::<traffic::Driver>();
    ecs.register_component::<vehicles::Stepping>();
    ecs.register_component::<crate::game::Player>();
    ecs.register_component::<crate::game::Bot>();
    ecs.register_component::<zombies::Zombie>();
    ecs.register_component::<civilians::Civilian>();
    ecs.register_component::<emergency::Medic>();
    ecs.register_component::<emergency::Officer>();
    ecs.register_component::<emergency::Wanted>();
    ecs.register_component::<ghosts::Ghost>();
    ecs.register_component::<limbs::Wounds>();
    ecs.register_component::<limbs::Struck>();
    ecs.register_component::<backpack::Carry>();
    ecs.register_component::<backpack::Lying>();
    ecs.register_component::<deliveries::Parcel>();
    ecs.register_component::<deliveries::LyingParcel>();
    projectiles::register(ecs);
    golf::register(ecs);
}

/// The mechanics a room runs, in tick order. `Titans` goes before `Lifts` and `Vehicles`, so that whoever
/// a fist holds boards nothing.
///
/// More on the order: `Bots` and `Zombies` go first: they write their input, which
/// everything after reads as it reads anyone's, and `Emergency` and `Buses` likewise for their people. `Emergency` goes before `Civilians`,
/// which keeps down those it has said an ambulance is coming for. `Traffic` goes before
/// `Vehicles`, which steps the cars it places and drives, and `Carwash` after it heals what that drove out of a wash. `Life` brings the dead back before `Npcs` and `Wildlife` move them.
/// `Ghosts` goes before `Movement`, so that a ghost that starts taking shape is not flown that tick.
pub fn default_mechanics() -> Vec<Box<dyn Mechanic>> {
    vec![Box::<Bots>::default(), Box::<Zombies>::default(), Box::<Emergency>::default(), Box::<Civilians>::default(), Box::<Buses>::default(), Box::new(Spawn), Box::<Titans>::default(), Box::<Lifts>::default(), Box::new(Pickups), Box::new(Submarines), Box::new(Drones), Box::new(Gates::seated()), Box::<Traffic>::default(), Box::new(Vehicles), Box::<Carwash>::default(), Box::<Gates>::default(), Box::new(Gunnery), Box::new(Ghosts), Box::new(Movement), Box::new(Breath), Box::<Mend>::default(), Box::new(Backpack), Box::new(Deliveries), Box::new(Life), Box::new(Npcs), Box::<Wildlife>::default(), Box::new(Combat), Box::new(Golf), Box::new(Structure), Box::new(Scores)]
}

/// What tests outside the mechanics reach in for: setting up a busy room without playing it.
#[cfg(test)]
pub(crate) mod for_tests {
    pub use super::life::{Harm, damage_entity};
    pub use super::drones::launch as launch_drone;
    pub use super::projectiles::Flying;

    pub fn add_vehicle(world: &mut crate::game::World, kind: u8, position: [f32; 3], yaw: f32) -> u32 {
        super::vehicles::add_vehicle(world, kind, position, yaw)
    }

    /// What the map editor would place for a test world with one road along X: `kinds` parked on its
    /// `spawns`, facing along the road, in the order given. The first two spawns are the far ends of the
    /// road, where planes stand with the whole road to take off along; every other kind takes the spawns
    /// from the seventh on (players start on the first six), then the third to the sixth.
    pub fn parked(spawns: &[[i32; 2]], kinds: &[u8]) -> Vec<shared::world::PlacedVehicle> {
        let mut ends = spawns[..2].iter().zip([-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2]);
        let mut middle = spawns[6..].iter().chain(spawns[2..6].iter().rev());
        kinds
            .iter()
            .map(|&kind| {
                if kind == shared::protocol::KIND_PLANE {
                    let (&at, yaw) = ends.next().expect("two planes at most");
                    shared::world::PlacedVehicle { kind, at, yaw }
                } else {
                    let &at = middle.next().expect("a spawn for every vehicle");
                    shared::world::PlacedVehicle { kind, at, yaw: -std::f32::consts::FRAC_PI_2 }
                }
            })
            .collect()
    }
}
