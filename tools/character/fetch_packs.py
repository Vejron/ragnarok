"""
Downloads the character models the game's people are made from into `.cache/characters/` (git-
ignored), where `build_character.py` reads them: Quaternius' Animated Men and Animated Women packs,
CC0, from poly.pizza.

    python3 tools/character/fetch_packs.py

A file already downloaded is kept. docs/CHARACTER-LOOKS.md has the rest of the pipeline.
"""

import os
import urllib.request

HERE = os.path.dirname(os.path.abspath(__file__))
CACHE = os.path.normpath(os.path.join(HERE, "../../.cache/characters"))

# (folder, file, poly.pizza model page, download), all CC0 1.0 by Quaternius.
MODELS = [
    ("Animated-Men-Pack", "Man.glb", "https://poly.pizza/m/fjHyMd5Wxw", "https://static.poly.pizza/985eacbf-9dde-44b7-9270-4e35c8400b13.glb"),
    ("Animated-Men-Pack", "Man_in_Long_Sleeves.glb", "https://poly.pizza/m/DLptRuewTn", "https://static.poly.pizza/e6019b9f-aed0-400c-8df4-ce5b648e9b82.glb"),
    ("Animated-Men-Pack", "Man_in_Suit.glb", "https://poly.pizza/m/mQnGoME1ez", "https://static.poly.pizza/66b57880-bcb0-479a-8d72-5c3e88afaa39.glb"),
    ("Animated-Women-Pack", "Woman.glb", "https://poly.pizza/m/AQsd9ngvKU", "https://static.poly.pizza/906a0cbe-b0a4-4312-b898-0f974b6f6771.glb"),
    ("Animated-Women-Pack", "Woman_Casual.glb", "https://poly.pizza/m/jpKRgGDxhk", "https://static.poly.pizza/51d5abdd-bb87-4b8d-9967-21738ffb8437.glb"),
    ("Animated-Women-Pack", "Woman_in_Dress.glb", "https://poly.pizza/m/zMyPlQXBzq", "https://static.poly.pizza/a642af96-e239-4c5f-b50d-7661ff51deec.glb"),
    ("Animated-Women-Pack", "Woman_in_Tank_Top.glb", "https://poly.pizza/m/XqzeZGB7iU", "https://static.poly.pizza/9a6a3e55-23ce-4d5c-89bc-7d3f30307ed0.glb"),
]

for folder, name, page, url in MODELS:
    path = os.path.join(CACHE, folder, name)
    if os.path.exists(path):
        print(f"have {folder}/{name}")
        continue
    os.makedirs(os.path.dirname(path), exist_ok=True)
    data = urllib.request.urlopen(urllib.request.Request(url, headers={"User-Agent": "ragnarok-asset-fetch"})).read()
    with open(path, "wb") as f:
        f.write(data)
    print(f"fetched {folder}/{name} ({len(data)} bytes) from {page}")
