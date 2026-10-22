//! The dynamic entities of a room as components in a `bevy_ecs` world, and the index from the wire id
//! (`NetId`) to the ECS `Entity`. Families move here from their own maps in `World` one at a time (see
//! MIGRATION.md); roosters are the first. Only `bevy_ecs`'s storage and queries are used: the ordered
//! `Mechanic` list is still the schedule.
//!
//! A thing is what its components say it is, not its kind: anything with a `Hurtbox` and `Health` can
//! be shot and blasted, anything with `Respawn` and `Home` comes back after dying, and `NetKind` only
//! tells the client which model to draw.

use bevy_ecs::query::{QueryData, QueryFilter, QueryIter, QueryState};
use std::any::{Any, TypeId};
use std::cell::{RefCell, RefMut};
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::collections::hash_map::Entry;

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

/// Whoever last hurt it, credited with its death. Absent while nobody has.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HurtBy(pub u32);

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
    index: HashMap<u32, Entity, BuildHasherDefault<IdHasher>>,
    /// The `QueryState` behind each kind of query asked for so far, kept between calls: making one
    /// matches it against every archetype, which cost more than the query itself.
    queries: RefCell<HashMap<TypeId, Box<dyn Any + Send>>>,
}

impl Default for Entities {
    fn default() -> Self {
        let mut ecs = bevy_ecs::world::World::new();
        // Registered up front so `try_query` on a shared borrow always finds them.
        ecs.register_component::<NetId>();
        ecs.register_component::<NetKind>();
        ecs.register_component::<Transform>();
        ecs.register_component::<Health>();
        ecs.register_component::<HurtBy>();
        ecs.register_component::<Hurtbox>();
        ecs.register_component::<Home>();
        ecs.register_component::<Respawn>();
        ecs.register_component::<DropOnDeath>();
        ecs.register_component::<Respawned>();
        crate::mechanics::register_components(&mut ecs);
        Entities { ecs, index: HashMap::default(), queries: RefCell::default() }
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

    /// Whether entity `id` has `Health` and none of it left: a dead rooster, a wrecked vehicle.
    pub fn dead(&self, id: u32) -> bool {
        self.get::<Health>(id).is_some_and(|health| !health.alive())
    }

    pub fn has<T: Component>(&self, id: u32) -> bool {
        self.get::<T>(id).is_some()
    }

    /// The kept `QueryState` for `D` and `F`, brought up to date with the archetypes made since it was
    /// last used. `None` while a component it needs is not registered.
    fn query<D: QueryData + 'static, F: QueryFilter + 'static>(&self) -> Option<RefMut<'_, QueryState<D, F>>> {
        let mut state = RefMut::filter_map(self.queries.borrow_mut(), |queries| kept::<D, F>(queries, &self.ecs)).ok()?;
        state.update_archetypes(&self.ecs);
        Some(state)
    }

    /// Hands `scan` every entity `D` and `F` match, in storage order (not id order: for what does not
    /// care, such as a count or a nearest-with-ties-broken-by-id). `None` while a component it needs is
    /// not registered.
    ///
    /// The state is taken out of the cache while `scan` runs, so `scan` may ask for other queries.
    pub fn scan<D: QueryData + 'static, F: QueryFilter + 'static, R>(&self, scan: impl FnOnce(QueryIter<'_, '_, D::ReadOnly, F>) -> R) -> Option<R> {
        let key = TypeId::of::<QueryState<D, F>>();
        let taken = self.queries.borrow_mut().remove(&key).and_then(|state| state.downcast::<QueryState<D, F>>().ok());
        let mut state = match taken {
            Some(state) => state,
            None => Box::new(QueryState::<D, F>::try_new(&self.ecs)?),
        };
        state.update_archetypes(&self.ecs);
        let result = scan(state.iter_manual(&self.ecs));
        self.queries.borrow_mut().insert(key, state);
        Some(result)
    }

    /// The same, to change what it is handed.
    pub fn scan_mut<D: QueryData + 'static, F: QueryFilter + 'static, R>(&mut self, scan: impl FnOnce(QueryIter<'_, '_, D, F>) -> R) -> Option<R> {
        let state = kept::<D, F>(self.queries.get_mut(), &self.ecs)?;
        Some(scan(state.iter_mut(&mut self.ecs)))
    }

    /// How many entities have a `T`.
    pub fn count<T: Component>(&self) -> usize {
        self.query::<&T, ()>().map_or(0, |state| state.iter_manual(&self.ecs).count())
    }

    /// Every entity's `T` with its id, in id order.
    pub fn all<T: Component>(&self) -> std::vec::IntoIter<(u32, &T)> {
        let mut all: Vec<(u32, &T)> = self.query::<(&NetId, &T), ()>().map_or_else(Vec::new, |state| state.iter_manual(&self.ecs).map(|(id, t)| (id.0, t)).collect());
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

    /// The ids of the entities whose `T` passes `keep`, in id order. Only those that pass are sorted,
    /// which for "whoever pressed fire" is usually none.
    pub fn ids_where<T: Component>(&self, mut keep: impl FnMut(u32, &T) -> bool) -> Vec<u32> {
        let mut ids = self.scan::<(&NetId, &T), (), _>(|all| all.filter(|&(id, t)| keep(id.0, t)).map(|(id, _)| id.0).collect::<Vec<u32>>()).unwrap_or_default();
        ids.sort_unstable();
        ids
    }

    /// The ids of the entities `F` matches, in id order: a pass that draws random numbers or makes
    /// output keeps the order the old `BTreeMap`s gave it.
    pub fn ids<F: QueryFilter + 'static>(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.query::<&NetId, F>().map_or_else(Vec::new, |state| state.iter_manual(&self.ecs).map(|id| id.0).collect());
        ids.sort_unstable();
        ids
    }
}

/// Hashes a wire id for the index. Ids are handed out one after another, so a multiply by an odd constant
/// (Fibonacci hashing) spreads them as well as SipHash does, at a fraction of the cost: every component
/// look-up by id goes through it.
#[derive(Default)]
struct IdHasher(u64);

impl Hasher for IdHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        for &byte in bytes {
            self.0 = (self.0.rotate_left(8) ^ u64::from(byte)).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        }
    }

    fn write_u32(&mut self, id: u32) {
        self.0 = u64::from(id).wrapping_mul(0x9E37_79B9_7F4A_7C15);
    }
}

/// The `QueryState` kept in `queries` for `D` and `F`, made the first time it is asked for.
fn kept<'q, D: QueryData + 'static, F: QueryFilter + 'static>(queries: &'q mut HashMap<TypeId, Box<dyn Any + Send>>, ecs: &bevy_ecs::world::World) -> Option<&'q mut QueryState<D, F>> {
    let state = match queries.entry(TypeId::of::<QueryState<D, F>>()) {
        Entry::Occupied(kept) => kept.into_mut(),
        Entry::Vacant(free) => free.insert(Box::new(QueryState::<D, F>::try_new(ecs)?)),
    };
    state.downcast_mut()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_query_sees_entities_of_archetypes_made_after_it() {
        let mut entities = Entities::default();
        entities.spawn(3, Health::full(10));
        assert_eq!(entities.ids::<With<Health>>(), [3]);
        assert_eq!(entities.count::<Health>(), 1);
        // New archetypes, after the queries were first kept.
        entities.spawn(1, (Health::full(5), HurtBy(9)));
        entities.spawn(2, (Health::full(5), Home([0.0; 2])));
        assert_eq!(entities.ids::<With<Health>>(), [1, 2, 3], "and in id order");
        assert_eq!(entities.count::<Health>(), 3);
        assert_eq!(entities.all::<Health>().map(|(id, health)| (id, health.hp)).collect::<Vec<_>>(), [(1, 5), (2, 5), (3, 10)]);
        entities.despawn(2);
        assert_eq!(entities.ids::<With<Health>>(), [1, 3]);
    }

    #[test]
    fn a_scan_may_ask_for_other_queries_and_scan_mut_changes_what_it_is_handed() {
        let mut entities = Entities::default();
        entities.spawn(1, Health::full(5));
        entities.spawn(2, Health::full(7));
        let nested = entities.scan::<&Health, (), _>(|outer| outer.map(|_| entities.scan::<&Health, (), _>(|inner| inner.count()).unwrap() + entities.count::<Health>()).sum::<usize>());
        assert_eq!(nested, Some(8));
        entities.scan_mut::<&mut Health, (), _>(|healths| healths.for_each(|mut health| health.hp -= 1));
        assert_eq!(entities.all::<Health>().map(|(_, health)| health.hp).collect::<Vec<_>>(), [4, 6]);
        assert_eq!(entities.scan::<&NetKind, (), _>(|kinds| kinds.count()), Some(0), "a registered component no one has");
    }
}
