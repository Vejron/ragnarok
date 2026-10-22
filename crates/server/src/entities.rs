//! The dynamic entities of a room as components in a `bevy_ecs` world, and the index from the wire id
//! (`NetId`) to the ECS `Entity`. Families move here from their own maps in `World` one at a time (see
//! MIGRATION.md); roosters are the first. Only `bevy_ecs`'s storage and queries are used: the ordered
//! `Mechanic` list is still the schedule.
//!
//! A thing is what its components say it is, not its kind: anything with a `Hurtbox` and `Health` can
//! be shot and blasted, anything with `Respawn` and `Home` comes back after dying, and `NetKind` only
//! tells the client which model to draw.

use bevy_ecs::query::QueryFilter;
use std::collections::HashMap;

pub use bevy_ecs::prelude::{Bundle, Component, Entity, Mut, With};

/// The id the wire knows an entity by. Shares the id space with players, vehicles, rockets and pickups.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct NetId(pub u32);

/// Which model the client draws for it (`KIND_*`). Never branched on by game logic.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetKind(pub u8);

/// Where it stands: its feet, and its facing (radians, 0 faces -Z).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Transform {
    pub position: [f32; 3],
    pub yaw: f32,
}

/// Health; zero is dead.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Health {
    pub hp: u8,
    pub max: u8,
}

impl Health {
    pub fn full(max: u8) -> Self {
        Health { hp: max, max }
    }

    pub fn alive(&self) -> bool {
        self.hp > 0
    }
}

/// What a round or a blast can meet: an upright capsule over the feet. `hit_kind` is the `HIT_*` the
/// shooter is told they hit.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Hurtbox {
    /// Height of the capsule's centre above the feet, metres.
    pub centre_y: f32,
    pub half_height: f32,
    pub radius: f32,
    pub hit_kind: u8,
}

impl Hurtbox {
    /// The capsule's centre for something standing at `feet`.
    pub fn centre(&self, feet: [f32; 3]) -> [f32; 3] {
        [feet[0], feet[1] + self.centre_y, feet[2]]
    }
}

/// Where it lives and comes back to, `[x, z]`, metres.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Home(pub [f32; 2]);

/// Comes back at `Home`, whole, `delay` ticks after dying; `left` counts down while dead.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Respawn {
    pub delay: u32,
    pub left: u32,
}

/// Leaves an item on the ground where it dies. A `unique` drop clears any earlier one of the same item
/// still lying there, so something killed over and over does not carpet the field.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DropOnDeath {
    pub item: u8,
    pub count: u8,
    pub unique: bool,
}

/// Put on an entity the tick it comes back, for whatever else it carries to start afresh; the
/// mechanic that owns that state takes it off.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Respawned;

/// The ECS world and the index from wire ids to entities.
pub struct Entities {
    pub ecs: bevy_ecs::world::World,
    index: HashMap<u32, Entity>,
}

impl Default for Entities {
    fn default() -> Self {
        let mut ecs = bevy_ecs::world::World::new();
        // Registered up front so `try_query` on a shared borrow always finds them.
        ecs.register_component::<NetId>();
        ecs.register_component::<NetKind>();
        ecs.register_component::<Transform>();
        ecs.register_component::<Health>();
        ecs.register_component::<Hurtbox>();
        ecs.register_component::<Home>();
        ecs.register_component::<Respawn>();
        ecs.register_component::<DropOnDeath>();
        ecs.register_component::<Respawned>();
        crate::mechanics::register_components(&mut ecs);
        Entities { ecs, index: HashMap::new() }
    }
}

impl Entities {
    /// Makes entity `id` (from `World::alloc_id`) out of `bundle`.
    pub fn spawn(&mut self, id: u32, bundle: impl Bundle) -> Entity {
        let entity = self.ecs.spawn((NetId(id), bundle)).id();
        let previous = self.index.insert(id, entity);
        debug_assert!(previous.is_none(), "net id {id} spawned twice");
        entity
    }

    /// Removes entity `id`, if there is one.
    pub fn despawn(&mut self, id: u32) -> bool {
        self.index.remove(&id).is_some_and(|entity| self.ecs.despawn(entity))
    }

    pub fn entity(&self, id: u32) -> Option<Entity> {
        self.index.get(&id).copied()
    }

    pub fn get<T: Component>(&self, id: u32) -> Option<&T> {
        self.ecs.get::<T>(self.entity(id)?)
    }

    pub fn get_mut<T: Component<Mutability = bevy_ecs::component::Mutable>>(&mut self, id: u32) -> Option<Mut<'_, T>> {
        let entity = self.entity(id)?;
        self.ecs.get_mut::<T>(entity)
    }

    /// Entity `id`'s `T`, which it must have, as indexing a map would.
    #[track_caller]
    pub fn of<T: Component>(&self, id: u32) -> &T {
        self.get::<T>(id).unwrap_or_else(|| panic!("entity {id} has no {}", std::any::type_name::<T>()))
    }

    /// The same, to change.
    #[track_caller]
    pub fn of_mut<T: Component<Mutability = bevy_ecs::component::Mutable>>(&mut self, id: u32) -> Mut<'_, T> {
        self.get_mut::<T>(id).unwrap_or_else(|| panic!("entity {id} has no {}", std::any::type_name::<T>()))
    }

    #[cfg(test)]
    pub fn has<T: Component>(&self, id: u32) -> bool {
        self.get::<T>(id).is_some()
    }

    /// How many entities have a `T`.
    pub fn count<T: Component>(&self) -> usize {
        self.ecs.try_query::<&T>().map_or(0, |mut query| query.iter(&self.ecs).count())
    }

    /// Every entity's `T` with its id, in id order.
    pub fn all<T: Component>(&self) -> std::vec::IntoIter<(u32, &T)> {
        let mut all: Vec<(u32, &T)> = self.ecs.try_query::<(&NetId, &T)>().map_or_else(Vec::new, |mut query| query.iter(&self.ecs).map(|(id, t)| (id.0, t)).collect());
        all.sort_unstable_by_key(|&(id, _)| id);
        all.into_iter()
    }

    pub fn insert(&mut self, id: u32, bundle: impl Bundle) {
        if let Some(entity) = self.entity(id) {
            self.ecs.entity_mut(entity).insert(bundle);
        }
    }

    pub fn remove<T: Bundle>(&mut self, id: u32) {
        if let Some(entity) = self.entity(id) {
            self.ecs.entity_mut(entity).remove::<T>();
        }
    }

    /// The ids of the entities `F` matches, in id order: a pass that draws random numbers or makes
    /// output keeps the order the old `BTreeMap`s gave it.
    pub fn ids<F: QueryFilter>(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.ecs.try_query_filtered::<&NetId, F>().map_or_else(Vec::new, |mut query| query.iter(&self.ecs).map(|id| id.0).collect());
        ids.sort_unstable();
        ids
    }
}
