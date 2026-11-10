"""Tiles the preview shots of one clip into a single contact sheet, to read a whole cycle at a glance.

  Blender -b -P tools/character/sheet.py -- DIR CLIP

Reads `DIR/CLIP_<0..5>.png` (side), `..._top.png` and `..._front.png`, as `build_character.py --preview`
writes them for the swimming clips, and writes `DIR/grid_CLIP.png`: one row a view, one column a
frame. It uses Blender's own numpy, so it needs nothing installed.
"""
import os
import sys

import bpy
import numpy as np

directory, clip = sys.argv[sys.argv.index("--") + 1 :][:2]
rows = []
for suffix in ("", "_top", "_front"):
    tiles = []
    for i in range(6):
        image = bpy.data.images.load(os.path.join(directory, f"{clip}_{i}{suffix}.png"))
        width, height = image.size
        tiles.append(np.array(image.pixels[:]).reshape(height, width, 4))
    rows.append(np.concatenate(tiles, axis=1))
# Blender's rows run bottom to top, so the first view goes last to end up on top.
sheet = np.concatenate(rows[::-1], axis=0)
height, width, _ = sheet.shape
out = bpy.data.images.new(clip, width, height, alpha=True)
out.pixels = sheet.ravel()
out.filepath_raw = os.path.join(directory, f"grid_{clip}.png")
out.file_format = "PNG"
out.save()
