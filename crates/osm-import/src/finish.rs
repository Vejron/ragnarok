//! What a building's OpenStreetMap tags say it looks like: the colour of its walls and of its roof
//! (`building:colour`, `roof:colour`) and what they are made of (`building:material`,
//! `roof:material`). Few buildings carry them; one that does is drawn as mapped, and the client
//! picks whatever is left unsaid.

use crate::osm::Tags;
use shared::world::{CLAD_BRICK, CLAD_RENDER, CLAD_SHEET, CLAD_STONE, CLAD_UNKNOWN, CLAD_WOOD, ROOFING_FELT, ROOFING_SHEET, ROOFING_SHINGLES, ROOFING_TILES, ROOFING_UNKNOWN};

/// A building's colours and materials as far as they are known: `Building::paint` and `Building::clad`.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Finish {
    /// `[wall, roof]` as `0xRRGGBB`, each where it is known.
    pub paint: [Option<u32>; 2],
    /// `[CLAD_*, ROOFING_*]`, 0 for each that is not known.
    pub clad: [u8; 2],
}

impl Finish {
    /// Both colours known (a shelter's, or what the aerial photo shows), and no material.
    pub fn painted(paint: Option<[u32; 2]>) -> Finish {
        Finish { paint: paint.map_or([None, None], |[wall, roof]| [Some(wall), Some(roof)]), clad: [CLAD_UNKNOWN, ROOFING_UNKNOWN] }
    }

    /// This finish, with what it leaves unsaid taken from `other`.
    pub fn or(self, other: Finish) -> Finish {
        let clad = |k: usize| if self.clad[k] != 0 { self.clad[k] } else { other.clad[k] };
        Finish { paint: [self.paint[0].or(other.paint[0]), self.paint[1].or(other.paint[1])], clad: [clad(0), clad(1)] }
    }
}

/// What `tags` say of a building's colours and materials.
pub fn of(tags: &Tags) -> Finish {
    let colour = |key: &str, names: fn(&str) -> Option<u32>| tags.get(key).and_then(|value| colour_of(value, names));
    let material = |keys: &[&str], kinds: fn(&str) -> u8| keys.iter().filter_map(|key| tags.get(*key)).map(|value| kinds(value.trim().to_lowercase().as_str())).find(|&kind| kind != 0).unwrap_or(0);
    Finish {
        paint: [colour("building:colour", wall_colour), colour("roof:colour", roof_colour)],
        clad: [material(&["building:material", "building:cladding", "building:facade:material"], cladding), material(&["roof:material"], roofing)],
    }
}

/// A mapped colour: `#rrggbb` or `#rgb` as it is written, or a name as `names` paints it. `None` for
/// anything else.
pub fn colour_of(colour: &str, names: fn(&str) -> Option<u32>) -> Option<u32> {
    let colour = colour.trim().to_lowercase();
    if let Some(hex) = colour.strip_prefix('#') {
        return match hex.len() {
            6 => u32::from_str_radix(hex, 16).ok(),
            3 => u32::from_str_radix(hex, 16).ok().map(|v| ((v >> 8) & 0xf) * 0x110000 + ((v >> 4) & 0xf) * 0x1100 + (v & 0xf) * 0x11),
            _ => None,
        };
    }
    names(colour.replace([' ', '_', '-'], "").as_str())
}

/// A wall's named colour as houses here are painted: red is falu red, yellow ochre, white the
/// off-white of linseed paint. They are the client's own house colours, so each is drawn with the
/// pattern that goes with it.
fn wall_colour(name: &str) -> Option<u32> {
    Some(match name {
        "red" | "darkred" | "firebrick" | "maroon" | "indianred" => 0xa8432f,
        "yellow" | "ochre" | "gold" | "khaki" | "lightyellow" => 0xd9b45a,
        "white" | "ivory" | "snow" | "whitesmoke" => 0xeee9dc,
        "beige" | "cream" | "tan" | "wheat" | "linen" => 0xd8c6a3,
        "grey" | "gray" | "lightgrey" | "lightgray" | "silver" => 0xc9c4b8,
        "darkgrey" | "darkgray" | "dimgrey" | "dimgray" => 0xa9a59c,
        "brown" | "saddlebrown" | "sienna" => 0x6e5642,
        "black" => 0x4a3a2e,
        "green" | "darkgreen" | "olive" | "lightgreen" => 0xb5bda4,
        "blue" | "lightblue" | "steelblue" | "lightsteelblue" => 0x9fb1c2,
        "orange" | "peachpuff" | "pink" | "salmon" => 0xe9c9b6,
        _ => return None,
    })
}

/// A roof's named colour: red is tile, black and grey felt or sheet, green copper gone dark.
fn roof_colour(name: &str) -> Option<u32> {
    Some(match name {
        "red" | "darkred" | "firebrick" | "maroon" | "orange" | "indianred" => 0xb5543f,
        "brown" | "saddlebrown" | "sienna" => 0x8d6e63,
        "black" => 0x3f4447,
        "darkgrey" | "darkgray" | "dimgrey" | "dimgray" => 0x5a5f63,
        "grey" | "gray" | "lightgrey" | "lightgray" | "silver" => 0x7d8388,
        "blue" | "darkblue" | "steelblue" | "slategrey" | "slategray" => 0x4f5d6b,
        "green" | "darkgreen" => 0x4a5a55,
        "white" => 0xc4c7cc,
        _ => return None,
    })
}

/// `building:material` as a `CLAD_*`, 0 for one that is not drawn in its own way.
fn cladding(material: &str) -> u8 {
    match material {
        "wood" | "timber" | "timber_framing" | "wooden" | "planks" | "log" | "logs" => CLAD_WOOD,
        "brick" | "bricks" | "clinker" | "masonry" => CLAD_BRICK,
        "plaster" | "plastered" | "render" | "stucco" | "concrete" | "cement_block" | "cement" | "eternit" | "mineral" => CLAD_RENDER,
        "stone" | "limestone" | "sandstone" | "granite" | "marble" => CLAD_STONE,
        "metal" | "steel" | "metal_plates" | "sheet" | "corrugated" | "tin" | "aluminium" => CLAD_SHEET,
        _ => CLAD_UNKNOWN,
    }
}

/// `roof:material` as a `ROOFING_*`, 0 for one that is not drawn in its own way.
fn roofing(material: &str) -> u8 {
    match material {
        "roof_tiles" | "tile" | "tiles" | "slate" | "clay" | "concrete_tiles" => ROOFING_TILES,
        "metal" | "metal_sheet" | "tin" | "copper" | "zinc" | "steel" | "sheet" | "aluminium" | "corrugated" => ROOFING_SHEET,
        "tar_paper" | "bitumen" | "roofing_felt" | "felt" | "asphalt" | "asphalt_shingle" | "concrete" | "gravel" | "membrane" | "eternit" => ROOFING_FELT,
        "wood" | "shingle" | "shingles" | "wooden_shingles" | "wood_shingles" => ROOFING_SHINGLES,
        _ => ROOFING_UNKNOWN,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tags(pairs: &[(&str, &str)]) -> Tags {
        pairs.iter().map(|(k, v)| (k.to_string(), v.to_string())).collect()
    }

    #[test]
    fn a_buildings_colours_and_materials_come_from_its_tags_where_it_has_them() {
        assert_eq!(of(&tags(&[("building", "house")])), Finish::default());
        let house = of(&tags(&[("building:colour", "Red"), ("building:material", "wood"), ("roof:material", "roof_tiles")]));
        assert_eq!(house, Finish { paint: [Some(0xa8432f), None], clad: [CLAD_WOOD, ROOFING_TILES] });
        let hall = of(&tags(&[("building:colour", "#8899AA"), ("roof:colour", "dark grey"), ("building:material", "metal"), ("roof:material", "metal")]));
        assert_eq!(hall, Finish { paint: [Some(0x8899aa), Some(0x5a5f63)], clad: [CLAD_SHEET, ROOFING_SHEET] });
    }

    #[test]
    fn a_colour_or_a_material_that_is_not_known_is_left_to_the_client() {
        assert_eq!(of(&tags(&[("building:colour", "mauve"), ("building:material", "glass"), ("roof:material", "thatch")])), Finish::default());
        // The cladding stands in where the material is one that is not drawn.
        assert_eq!(of(&tags(&[("building:material", "glass"), ("building:cladding", "brick")])).clad, [CLAD_BRICK, ROOFING_UNKNOWN]);
    }

    #[test]
    fn what_the_tags_leave_unsaid_is_taken_from_the_photo() {
        let tagged = of(&tags(&[("building:colour", "white")]));
        assert_eq!(tagged.or(Finish::painted(Some([0x111111, 0x222222]))).paint, [Some(0xeee9dc), Some(0x222222)]);
        assert_eq!(tagged.or(Finish::painted(None)).paint, [Some(0xeee9dc), None]);
    }
}
