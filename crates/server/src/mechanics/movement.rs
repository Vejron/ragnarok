use super::Mechanic;
use crate::entities::With;
use crate::game::{Player, World};
use shared::consts::{BACKPACK_SPEED, BUTTON_LAMP, BUTTON_SKIS, SKI_CLIP_SPEED};
use shared::items::{aim_speed_scale, fills_hands, hotbar_index, ITEM_NONE};
use shared::protocol::LIMB_LEGS;
use shared::world::THEME_WINTER;

/// Walks every standing soldier by its input through the character controller, and keeps track of
/// the car it stands on, if any.
///
/// Riders are carried by `Vehicles` (before this runs), so here they only walk about on the roof.
/// Leaving a car by jumping or stepping off keeps its speed as knockback, so a soldier does not
/// drop away from a moving car as if it had stopped.
pub struct Movement;

impl Mechanic for Movement {
    fn on_tick(&mut self, world: &mut World, dt: f32) {
        let winter = world.map.theme == THEME_WINTER;
        for id in world.entities.ids::<With<Player>>() {
            let alive = world.entities.alive(id);
            let lost = super::limbs::lost(world, id);
            let carrying = super::backpack::carries(world, id);
            let mut player = world.entities.of_mut::<Player>(id);
            let player = &mut *player;
            let lamp = player.input.buttons() & BUTTON_LAMP != 0;
            player.lamp ^= lamp && !player.lamp_held;
            player.lamp_held = lamp;
            let skis = player.input.buttons() & BUTTON_SKIS != 0;
            let switch_skis = skis && !player.ski_held;
            player.ski_held = skis;
            // A lift's rider is carried by `Lifts`, which ran before this.
            if player.seat.is_some() || player.lift.is_some() || !alive {
                continue;
            }
            let character = &mut player.character;
            character.lost = lost;
            character.skis &= lost & LIMB_LEGS == 0;
            if switch_skis && winter && lost & LIMB_LEGS == 0 && character.knock[0].hypot(character.knock[1]) < SKI_CLIP_SPEED {
                character.skis = !character.skis;
                character.knock = [0.0, 0.0];
            }
            let velocity = world.physics.car_pose(character.ride).map(|pose| pose.velocity);
            let item = hotbar_index(player.input.slot()).map_or(ITEM_NONE, |slot| player.hands.hotbar[slot].item);
            let scale = aim_speed_scale(item, player.input.buttons()) * if carrying { BACKPACK_SPEED } else { 1.0 };
            world.physics.step_character(character, &player.input, dt, scale, fills_hands(item), velocity);
            character.ride = if character.grounded { world.physics.car_under(character.position).map_or(0, |hit| hit.car) } else { 0 };
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::game::{Command, Game, Player};
    use shared::consts::*;
    use shared::items::{ITEM_AK5, SLOT_NONE, STANDARD_LOADOUT};
    use shared::protocol::{InputFrame, LIMB_LEGS};
    use shared::world::{THEME_WINTER, WORLD_VERSION, WorldFile};
    use tokio::sync::{mpsc, oneshot};

    fn game(theme: u8) -> (Game, u32, mpsc::Receiver<crate::game::Outgoing>) {
        let map = WorldFile { version: WORLD_VERSION, half_extent: 20_000, spawns: vec![[0, 0]], ..Default::default() };
        let mut game = Game::new(map, 0).unwrap();
        game.world.map.theme = theme;
        game.world.clear_pickups();
        let (outbox, inbox) = mpsc::channel(4096);
        let (reply, mut response) = oneshot::channel();
        game.handle(Command::Join { outbox, reply, profile: Default::default() });
        (game, response.try_recv().unwrap().unwrap(), inbox)
    }

    fn press(game: &mut Game, id: u32, seq: &mut u32, buttons: u16, slot: u8, ticks: u32) {
        for _ in 0..ticks {
            *seq += 1;
            game.handle(Command::Input(id, 0, vec![InputFrame { seq: *seq, strafe: 0, forward: 0, yaw: 0, buttons, pitch: 0, view: 0, slot }]));
            game.step();
        }
    }

    fn on_skis(game: &Game, id: u32) -> bool {
        game.world.entities.of::<Player>(id).character.skis
    }

    #[test]
    fn the_ski_button_switches_them_once_a_press() {
        let (mut game, id, _inbox) = game(THEME_WINTER);
        let mut seq = 0;
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 10);
        assert!(on_skis(&game, id), "held for ten ticks, it switched once");
        press(&mut game, id, &mut seq, 0, 0, 2);
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 2);
        assert!(!on_skis(&game, id), "and off again");
    }

    #[test]
    fn skis_stay_on_at_speed() {
        let (mut game, id, _inbox) = game(THEME_WINTER);
        let mut seq = 0;
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 2);
        press(&mut game, id, &mut seq, 0, 0, 1);
        game.world.entities.of_mut::<Player>(id).character.knock = [SKI_CLIP_SPEED + 3.0, 0.0];
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 1);
        assert!(on_skis(&game, id));
    }

    #[test]
    fn skis_are_for_winter_worlds_and_people_with_legs() {
        let (mut game, id, _inbox) = game(0);
        let mut seq = 0;
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 2);
        assert!(!on_skis(&game, id), "no snow");

        let (mut game, id, _inbox) = self::game(THEME_WINTER);
        game.world.entities.insert(id, super::super::limbs::Wounds { lost: LIMB_LEGS, ..Default::default() });
        press(&mut game, id, &mut seq, BUTTON_SKIS, 0, 2);
        assert!(!on_skis(&game, id), "no legs");
    }

    #[test]
    fn a_winter_soldier_keeps_the_whole_loadout() {
        let (game, id, _inbox) = game(THEME_WINTER);
        assert_eq!(game.world.entities.of::<Player>(id).hands.hotbar.map(|slot| slot.item), STANDARD_LOADOUT);
    }

    #[test]
    fn empty_hands_hold_nothing_and_fire_nothing() {
        let (mut game, id, _inbox) = game(0);
        let mut seq = 0;
        assert_eq!(game.world.entities.of::<Player>(id).held().item, ITEM_AK5);
        let ammo = game.world.entities.of::<Player>(id).hands.hotbar[0].ammo;
        press(&mut game, id, &mut seq, BUTTON_FIRE, SLOT_NONE, 5);
        assert_eq!(game.world.entities.of::<Player>(id).held().count, 0);
        assert_eq!(game.world.entities.of::<Player>(id).hands.hotbar[0].ammo, ammo);
        press(&mut game, id, &mut seq, 0, 0, 5);
        assert_eq!(game.world.entities.of::<Player>(id).held().item, ITEM_AK5, "the same gun comes back");
    }
}
