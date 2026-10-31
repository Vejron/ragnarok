//! What winter does to a world in the importer.

use shared::world::{NO_WATER, Tile};

/// How far under the water surface a frozen lake's ground ends, centimetres. Depth stays positive, so
/// the client still draws the surface, as ice, exactly where it drew water.
const ICE_SKIN: i32 = 3;

/// Freezes the lakes and rivers: the ground rises to just under the surface wherever it was carved
/// out, so what is walked, driven and skied on is flat and nothing needs to know about ice.
pub fn freeze<'a>(tiles: impl IntoIterator<Item = &'a mut Tile>) {
    for tile in tiles {
        if tile.water.len() != tile.ground.len() {
            continue;
        }
        for (ground, &water) in tile.ground.iter_mut().zip(&tile.water) {
            if water != NO_WATER {
                *ground = (*ground).max(water - ICE_SKIN);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_lake_bed_rises_to_the_ice_and_the_shore_stays() {
        let mut tile = Tile { ground: vec![-500, 90, 50], water: vec![100, 100, NO_WATER], ..Default::default() };
        let mut high = Tile { ground: vec![120], water: vec![100], ..Default::default() };
        freeze([&mut tile, &mut high]);
        assert_eq!(tile.ground, [97, 97, 50]);
        assert_eq!(high.ground, [120]);
    }
}
