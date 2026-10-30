//! The player's own settings, sent as query parameters on the `/transport` URL when they connect:
//! `name=Åsa&color=3&spawn=1200,-4500&spawn=...&loadout=2,5,0,8,11`. Set on the client's `/settings.html` page.

use shared::consts::{MAX_NAME_CHARS, MAX_SPAWNS, PLAYER_COLOR_COUNT};
use shared::items::{HOTBAR_SLOTS, ITEM_NONE, can_start_with};

/// What a player chose, cleaned up. Spawns are not checked against the world here; `Game::join`
/// drops the ones that are not usable.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Profile {
    /// Empty when none was given; the game then names the player after their id.
    pub name: String,
    /// Below `PLAYER_COLOR_COUNT`, or `None` for the colour picked by id.
    pub color: Option<u8>,
    /// Spawn points `[x, z]`, centimetres, in the order they are used. At most `MAX_SPAWNS`.
    pub spawns: Vec<[i32; 2]>,
    /// The item in each hotbar slot at the start of a life, `ITEM_NONE` for an empty one; each item at
    /// most once. `None` for the standard loadout.
    pub loadout: Option<[u8; HOTBAR_SLOTS]>,
}

/// Characters that are invisible or reorder text, which a name could use to hide or spoof another.
fn hidden(c: char) -> bool {
    c.is_control() || matches!(c, '\u{200B}'..='\u{200F}' | '\u{202A}'..='\u{202E}' | '\u{2060}'..='\u{2069}' | '\u{FEFF}')
}

/// `name` without hidden characters or runs of spaces, trimmed and cut to `MAX_NAME_CHARS`.
pub fn clean_name(name: &str) -> String {
    // Tabs and newlines are control characters too, so they become spaces before the rest go.
    let visible: String = name.chars().map(|c| if c.is_whitespace() { ' ' } else { c }).filter(|&c| !hidden(c)).collect();
    visible.split_whitespace().collect::<Vec<_>>().join(" ").chars().take(MAX_NAME_CHARS).collect::<String>().trim_end().to_string()
}

/// `2,5,0,8,11`, one item a slot. `None` unless it is a loadout a player can choose as a whole.
fn parse_loadout(value: &str) -> Option<[u8; HOTBAR_SLOTS]> {
    let mut items = [ITEM_NONE; HOTBAR_SLOTS];
    let mut values = value.split(',');
    for slot in 0..HOTBAR_SLOTS {
        let item = values.next()?.trim().parse::<u8>().ok()?;
        if item != ITEM_NONE && (!can_start_with(item) || items.contains(&item)) {
            return None;
        }
        items[slot] = item;
    }
    values.next().is_none().then_some(items)
}

impl Profile {
    /// Reads the query parameters, ignoring any it does not know and any value that does not parse.
    pub fn from_query<'a>(pairs: impl IntoIterator<Item = (&'a str, &'a str)>) -> Self {
        let mut profile = Profile::default();
        for (key, value) in pairs {
            match key {
                "name" => profile.name = clean_name(value),
                "color" => profile.color = value.parse::<u8>().ok().filter(|&color| color < PLAYER_COLOR_COUNT),
                "loadout" => profile.loadout = parse_loadout(value),
                "spawn" if profile.spawns.len() < MAX_SPAWNS => {
                    if let Some((x, z)) = value.split_once(',')
                        && let (Ok(x), Ok(z)) = (x.trim().parse::<i32>(), z.trim().parse::<i32>())
                    {
                        profile.spawns.push([x, z]);
                    }
                }
                _ => {}
            }
        }
        profile
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_full_profile_is_read() {
        let profile = Profile::from_query([("name", "Åsa"), ("color", "3"), ("spawn", "1200,-4500"), ("spawn", "0,7"), ("loadout", "2,0,13,8,11"), ("class", "medic")]);
        assert_eq!(profile, Profile { name: "Åsa".into(), color: Some(3), spawns: vec![[1200, -4500], [0, 7]], loadout: Some([2, 0, 13, 8, 11]) });
    }

    #[test]
    fn nothing_given_is_the_default() {
        assert_eq!(Profile::from_query([]), Profile::default());
    }

    #[test]
    fn names_lose_hidden_characters_and_are_cut() {
        assert_eq!(clean_name("  Bo\u{0}b \n\t  Ek "), "Bob Ek");
        assert_eq!(clean_name("\u{202E}evil\u{200B}"), "evil");
        assert_eq!(clean_name("abcdefghijklmnopqrstuvwxyz"), "abcdefghijklmnop");
        assert_eq!(clean_name("abcdefghijklmno pq"), "abcdefghijklmno", "no trailing space after the cut");
        assert_eq!(clean_name(" \u{7} "), "");
    }

    #[test]
    fn a_colour_out_of_range_or_junk_is_dropped() {
        assert_eq!(Profile::from_query([("color", "12")]).color, None);
        assert_eq!(Profile::from_query([("color", "-1")]).color, None);
        assert_eq!(Profile::from_query([("color", "red")]).color, None);
        assert_eq!(Profile::from_query([("color", "11")]).color, Some(11));
    }

    #[test]
    fn a_loadout_that_cannot_be_chosen_is_dropped_whole() {
        for junk in ["", "2,5,8", "2,5,10,8,11,1", "2,5,x,8,11", "2,2,10,8,11", "2,6,10,8,11", "2,14,10,8,11", "2,300,10,8,11"] {
            assert_eq!(Profile::from_query([("loadout", junk)]).loadout, None, "{junk:?}");
        }
        assert_eq!(Profile::from_query([("loadout", "0,0,0,0,0")]).loadout, Some([0; HOTBAR_SLOTS]), "empty slots may repeat");
    }

    #[test]
    fn spawns_that_do_not_parse_are_skipped_and_at_most_five_kept() {
        let junk = Profile::from_query([("spawn", "1,2"), ("spawn", "x,2"), ("spawn", "3"), ("spawn", "99999999999,0"), ("spawn", " 4 , 5 ")]);
        assert_eq!(junk.spawns, vec![[1, 2], [4, 5]]);
        let many: Vec<String> = (0..9).map(|i| format!("{i},{i}")).collect();
        let profile = Profile::from_query(many.iter().map(|value| ("spawn", value.as_str())));
        assert_eq!(profile.spawns.len(), MAX_SPAWNS);
        assert_eq!(profile.spawns[4], [4, 4]);
    }
}
