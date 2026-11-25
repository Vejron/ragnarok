# Models

Every model in this folder is built by a script in `tools/` (Blender, headless) and is our own, except:

- `deer.glb`, `stag.glb`: "Deer" and "Stag" by Quaternius, from his Ultimate Animated Animals — https://poly.pizza/m/T6Cs7tmMHJ and https://poly.pizza/m/tQdzbZ1Cmw — rescaled, turned and cut down to the clips the game plays by `tools/npc/import_deer.py`.

- `player.glb`'s people: "Man", "Man in Long Sleeves", "Man in Suit", "Woman", "Woman Casual", "Woman in Dress" and "Woman in Tank Top" by Quaternius, from his Animated Men and Animated Women packs on poly.pizza (the links are in `tools/character/fetch_packs.py`), recoloured into the game's looks with a helmet, vests and caps added, bound to one skeleton, and moved by the game's own clips and by clips from Quaternius' Universal Animation Library 1 and 2, the free Standard tier (https://quaternius.itch.io/universal-animation-library, https://quaternius.itch.io/universal-animation-library-2), all CC0 (`tools/character/build_character.py`, CHARACTER-LOOKS.md in docs/).

All are CC0 1.0 (public domain); no attribution is required, it is given as a courtesy.
