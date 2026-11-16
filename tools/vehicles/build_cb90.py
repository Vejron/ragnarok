"""
Builds the Stridsbåt 90 (the Swedish fast assault craft) and exports it to client/public/models/cb90.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_cb90.py -- \
        [--out client/public/models/cb90.glb] [--preview DIR]

Everything is procedural and flat-shaded, a few thousand triangles in all.

Conventions (as build_pbv.py)
  * Blender: Z up, the bow points along +Y, the boat's right hand side is +X. The glTF exporter turns that
    into Y up, bow along -Z, which is the game's vehicle frame, so a game point (x, y, z) is Blender
    (x, -z, y).
  * Origin is the centre of the hull's bounding box (3.8 x 2.0 x 15.9 m, the real boat's size,
    CB90_HALF_EXTENTS). The waterline at rest is CB90_DRAFT (0.8 m) over the keel, 0.2 m below it.
  * The shape is the real one's: a deep-V planing hull that tapers to a pointed bow over CB90_BOW, with a
    rubbing strake along its sheer and two waterjet nozzles in the transom; the low troop compartment
    over the fore deck with the bow ramp ahead of it; the wheelhouse behind it with its raked windscreen,
    and the cannon's turret on its roof with the radar mast behind; the open after deck over the engines,
    with a bench either side for the riders and a rail round it; and the twin machine guns on a post at
    the front of the troop compartment's roof.
  * Nodes the game looks up by name:
      `hull`     everything fixed to the hull
      `turret`   origin on the turret's ring (the game's CB90_TURRET_PIVOT, the middle of its solid); the
                 game yaws it
      `gun`      child of `turret`, origin on the ring too; the game pitches it and slides it back in recoil
      `mg`       the bow guns' cradle, origin on their pivot (CB90_MG_PIVOT); the game yaws it
      `mg_gun`   child of `mg`, origin on the pivot too, the two barrels either side; the game pitches it
  * Materials: `paint` is camouflaged per boat at runtime; `dark` (glass, fenders, nozzles), `metal`
    (guns, rails, fittings) and `steel` (lamps) keep their colours.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import MATERIALS, Part, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/cb90.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with CB90_* in crates/shared/src/vehicles/cb90.rs.
# --------------------------------------------------------------------------------------------------
HALF_BEAM, HALF_HEIGHT, HALF_LENGTH = 1.9, 1.0, 7.95
BOW = 4.5  # the sides taper to the stem over this much (CB90_BOW)
DECK = HALF_HEIGHT
KEEL = -HALF_HEIGHT
WATERLINE = KEEL + 0.8
# The superstructure's side outline as (y, z), bow first: its glacis rising from the fore deck to the
# low roof over the troops, the wheelhouse's raked windscreen up to its roof, and its back. Its sides
# lean in from SUPER_BASE_W at the deck to SUPER_TOP_W at the wheelhouse's roof.
SUPER_BASE, SUPER_TOP = 0.95, 2.9
SUPER_BASE_W, SUPER_TOP_W = 1.65, 1.15
TROOP_ROOF = 2.0
SUPER = [(4.3, SUPER_BASE), (3.0, TROOP_ROOF), (2.2, TROOP_ROOF), (0.6, TROOP_ROOF), (-0.3, SUPER_TOP), (-3.9, SUPER_TOP), (-4.4, 2.6), (-4.4, SUPER_BASE), (2.2, SUPER_BASE)]
TURRET_PIVOT = Vector((0.0, -1.3, 3.2))  # game [0, 3.2, 1.3]
TURRET_RADIUS, TURRET_HALF_HEIGHT = 0.6, 0.3
CANNON_LENGTH = 2.3
MG_PIVOT = Vector((0.0, 2.8, 2.45))  # game [0, 2.45, -2.8]
MG_BARREL_X = 0.16
MG_LENGTH = 1.1

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)

MATERIALS[:] = [
    material("paint", (0.32, 0.38, 0.3)),
    material("dark", (0.08, 0.09, 0.1)),
    material("metal", (0.22, 0.24, 0.22)),
    material("steel", (0.6, 0.6, 0.56)),
    material("buoy", (0.9, 0.3, 0.08)),
]
PAINT, DARK, METAL, STEEL, BUOY = range(5)


def sheer(y):
    """The deck's height at `y`: level aft, climbing towards the stem."""
    s = max(0.0, (y - 1.0) / (HALF_LENGTH - 1.0))
    return DECK + 0.7 * s**1.6


def section(y):
    """The hull's cross-section at `y` as (x, z) corners from port deck edge round under the keel to
    starboard: deck edge, chine, keel, chine, deck edge. Forward the beam narrows to the stem, the sides
    flare out above the chine and the forefoot rises into a raked stem."""
    t = max(0.0, (y - (HALF_LENGTH - BOW)) / BOW)  # 0 abaft the bow, 1 at the stem
    k = max(0.0, (y - 2.0) / (HALF_LENGTH - 2.0))
    beam = HALF_BEAM * (1.0 - 0.95 * max(0.0, y / HALF_LENGTH) ** 2.2)
    keel = KEEL + 1.6 * k**1.8
    deck = sheer(y)
    chine = min(keel + 0.4 + 0.3 * t, deck - 0.2)
    flare = 0.88 - 0.25 * t
    return [(-beam, deck), (-beam * flare, chine), (0.0, keel), (beam * flare, chine), (beam, deck)]


def lean(z, y=0.0, pad=0.0):
    """Half the superstructure's width at height `z` and `y`: its sides lean in, and its front narrows to
    follow the bow. `pad` stands proud of them."""
    f = (z - SUPER_BASE) / (SUPER_TOP - SUPER_BASE)
    narrow = 1.0 - 0.38 * max(0.0, (y - 2.2) / (SUPER[0][0] - 2.2))
    return (SUPER_BASE_W + (SUPER_TOP_W - SUPER_BASE_W) * f) * narrow + pad


def leaning(part, profile, mat, pad=0.0):
    """Extrudes a closed (y, z) outline across X with the superstructure's leaning sides."""
    a = [part.bm.verts.new((-lean(z, y, pad), y, z)) for y, z in profile]
    b = [part.bm.verts.new((lean(z, y, pad), y, z)) for y, z in profile]
    sides = [part.bm.faces.new(a), part.bm.faces.new(b[::-1])]
    faces = list(sides)
    for i in range(len(profile)):
        j = (i + 1) % len(profile)
        faces.append(part.bm.faces.new((a[i], a[j], b[j], b[i])))
    for face in faces:
        face.material_index = mat
    bmesh.ops.triangulate(part.bm, faces=sides)


# --------------------------------------------------------------------------------------------------
# Hull
# --------------------------------------------------------------------------------------------------
STATIONS = [-HALF_LENGTH, -6.0, -3.0, 0.0, 1.5, 3.0, 4.3, 5.3, 6.2, 6.9, 7.5, HALF_LENGTH]

hull = Part("hull")
rings = [[hull.bm.verts.new((x, y, z)) for x, z in section(y)] for y in STATIONS]
for a, b in zip(rings, rings[1:]):
    for i in range(len(a) - 1):
        hull.bm.faces.new((a[i], a[i + 1], b[i + 1], b[i])).material_index = PAINT
    # The deck between the two edges.
    hull.bm.faces.new((a[-1], a[0], b[0], b[-1])).material_index = PAINT
hull.bm.faces.new(rings[0][::-1]).material_index = PAINT  # the transom
hull.bm.faces.new(rings[-1]).material_index = PAINT  # the stem

# The rubbing strake along the sheer, abaft the bow, and the waterjet nozzles in the transom with their
# reversing buckets over them.
for side in (-1, 1):
    hull.box((side * (HALF_BEAM + 0.05), (0.5 - HALF_LENGTH) / 2, DECK - 0.15), (0.12, 0.5 + HALF_LENGTH, 0.2), DARK)
    hull.cylinder((side * 0.8, -HALF_LENGTH - 0.2, WATERLINE - 0.25), 0.28, 0.4, DARK, axis="Y", segments=8)
    hull.box((side * 0.8, -HALF_LENGTH - 0.42, WATERLINE + 0.05), (0.62, 0.12, 0.3), METAL)

# --------------------------------------------------------------------------------------------------
# Superstructure: one long, low, faceted house, its sides leaning in: the troops' compartment under the
# low roof forward, the wheelhouse raised behind it.
# --------------------------------------------------------------------------------------------------
leaning(hull, SUPER, PAINT)
# The wheelhouse's windows: a band round its sides and back, and the raked windscreen.
leaning(hull, [(-0.75, 2.3), (-0.75, 2.7), (-3.7, 2.7), (-3.7, 2.3)], DARK, pad=0.02)
for y in (-4.41,):
    hull.box((0.0, y, 2.55), (1.6, 0.03, 0.35), DARK)
slope = Vector((-0.9, 0.9)).normalized()
normal = Vector((slope.y, -slope.x))  # out of the windscreen, forward and up
low = Vector((0.6, TROOP_ROOF)) + slope * 0.3
high = Vector((0.6, TROOP_ROOF)) + slope * 1.1
out = normal * 0.03
hull.prism([tuple(low + out), tuple(high + out), tuple(high - out), tuple(low - out)], -1.05, 1.05, DARK)
# Portholes along the troops' compartment, and a door either side at the back of the wheelhouse.
for y in (0.2, 1.0, 1.8):
    leaning(hull, [(y + 0.25, 1.45), (y + 0.25, 1.7), (y - 0.25, 1.7), (y - 0.25, 1.45)], DARK, pad=0.02)
leaning(hull, [(-3.2, 1.05), (-3.2, 2.15), (-3.9, 2.15), (-3.9, 1.05)], METAL, pad=0.015)
# The roof hatch over the troops, and the bow guns' post at the front of their roof.
hull.box((0.0, 1.6, TROOP_ROOF + 0.03), (1.0, 1.0, 0.06), PAINT)
hull.cylinder((MG_PIVOT.x, MG_PIVOT.y, (TROOP_ROOF + MG_PIVOT.z) / 2 - 0.1), 0.09, MG_PIVOT.z - TROOP_ROOF - 0.2, METAL, segments=6)
hull.cylinder((MG_PIVOT.x, MG_PIVOT.y, TROOP_ROOF + 0.04), 0.35, 0.08, METAL, segments=8)
# On the wheelhouse roof: the turret's ring, a searchlight each side, and the mast at the back with the
# radar's bar and the lamps' yard.
hull.cylinder((TURRET_PIVOT.x, TURRET_PIVOT.y, SUPER_TOP + 0.02), 0.7, 0.04, METAL, segments=10)
for side in (-1, 1):
    hull.cylinder((side * 0.8, -0.8, SUPER_TOP + 0.12), 0.1, 0.2, STEEL, axis="Y", segments=8)
hull.box((0.0, -3.5, SUPER_TOP + 0.9), (0.18, 0.18, 1.8), METAL)
hull.box((0.0, -3.5, SUPER_TOP + 1.3), (1.6, 0.2, 0.14), PAINT)
hull.box((0.0, -3.5, SUPER_TOP + 1.85), (0.9, 0.06, 0.06), METAL)
hull.cylinder((0.0, -3.5, SUPER_TOP + 2.3), 0.03, 0.8, METAL, segments=4)
# A life ring either side of the wheelhouse, on its leaning wall.
for side in (-1, 1):
    hull.cylinder((side * (lean(1.7, -2.2) + 0.06), -2.2, 1.7), 0.3, 0.08, BUOY, axis="X", segments=10)

# --------------------------------------------------------------------------------------------------
# The decks: the bow ramp folded flat on the fore deck, bollards, and on the after deck the engines'
# hatch, a bench either side for the riders and a rail round it all.
# --------------------------------------------------------------------------------------------------
hull.box((0.0, 5.3, sheer(5.3) + 0.04), (1.1, 1.4, 0.06), PAINT)
hull.box((0.0, 4.6, sheer(4.6) + 0.06), (1.2, 0.12, 0.1), METAL)
aft_front, aft_back = -4.4, -HALF_LENGTH + 0.3
hull.box((0.0, -6.3, DECK + 0.03), (1.6, 2.0, 0.06), PAINT)
for side in (-1, 1):
    hull.box((side * 0.9, -5.8, DECK + 0.22), (0.5, 1.4, 0.44), PAINT)
    for k in range(4):
        y = aft_back + k * (aft_front - aft_back) / 3
        hull.cylinder((side * (HALF_BEAM - 0.12), y, DECK + 0.45), 0.025, 0.9, METAL, segments=4)
    hull.cylinder((side * (HALF_BEAM - 0.12), (aft_front + aft_back) / 2, DECK + 0.9), 0.03, aft_front - aft_back, METAL, axis="Y", segments=4)
    for y in (5.9, -HALF_LENGTH + 0.6):
        hull.cylinder((side * 0.6 if y > 0 else side * 1.3, y, sheer(y) + 0.12), 0.09, 0.24, METAL, segments=6)
hull.cylinder((0.0, aft_back, DECK + 0.9), 0.03, 2 * HALF_BEAM - 0.24, METAL, axis="X", segments=4)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# The turret: a squat drum leaning in to a flat roof, with the sight's head; the cannon out of a
# cradle in its face, a long thin barrel with a flash hider.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=TURRET_PIVOT)
BASE, ROOF = -TURRET_HALF_HEIGHT, TURRET_HALF_HEIGHT
turret.cylinder((0, 0, 0), TURRET_RADIUS, ROOF - BASE, PAINT, segments=10, radius2=0.48)
turret.box((0.3, 0.2, ROOF + 0.1), (0.2, 0.25, 0.2), PAINT)
turret.box((0.3, 0.33, ROOF + 0.1), (0.14, 0.02, 0.12), DARK)
turret.box((-0.25, -0.2, ROOF + 0.02), (0.4, 0.4, 0.04), PAINT)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 0.55, 0.0), (0.3, 0.45, 0.28), PAINT)
gun.cylinder((0.0, 0.95, 0.0), 0.08, 0.5, METAL, axis="Y", segments=6)
gun.cylinder((0.0, (1.2 + CANNON_LENGTH) / 2, 0.0), 0.045, CANNON_LENGTH - 1.2, METAL, axis="Y", segments=6, radius2=0.04)
gun.cylinder((0.0, CANNON_LENGTH - 0.1, 0.0), 0.06, 0.2, METAL, axis="Y", segments=6)
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# The bow guns: a cradle with a small shield on the post, and two Ksp 58s side by side with their
# ammunition boxes.
# --------------------------------------------------------------------------------------------------
mg = Part("mg", location=MG_PIVOT)
mg.box((0.0, 0.0, -0.12), (0.5, 0.3, 0.1), METAL)
mg.box((0.0, 0.32, 0.12), (0.75, 0.04, 0.42), PAINT, rx=-0.15)
mg_obj = mg.build()

mg_gun = Part("mg_gun", parent=mg_obj)
for side in (-1, 1):
    x = side * MG_BARREL_X
    mg_gun.box((x, 0.0, 0.0), (0.09, 0.7, 0.14), METAL)
    mg_gun.cylinder((x, (0.35 + MG_LENGTH) / 2, 0.0), 0.022, MG_LENGTH - 0.35, METAL, axis="Y", segments=5)
    mg_gun.cylinder((x, MG_LENGTH - 0.05, 0.0), 0.035, 0.1, METAL, axis="Y", segments=5)
    mg_gun.box((x + side * 0.12, -0.05, -0.05), (0.14, 0.25, 0.18), PAINT)
mg_gun.box((0.0, -0.3, 0.02), (0.36, 0.12, 0.06), METAL)  # the handles between them
mg_gun_obj = mg_gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, turret_obj, gun_obj, mg_obj, mg_gun_obj]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    scene = bpy.context.scene
    os.makedirs(PREVIEW, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 1200, 600
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    middle = 1.0
    views = {
        "side": ("ORTHO", Vector((30, 0, middle)), Vector((0, 0, middle)), 19.0),
        "top": ("ORTHO", Vector((0, 0, 30)), Vector((0, 0, 0)), 19.0),
        "front": ("ORTHO", Vector((0, 30, middle)), Vector((0, 0, middle)), 8.0),
        "three": ("PERSP", Vector((-17, 17, 7)), Vector((0, 0.5, middle)), 0),
        "rear": ("PERSP", Vector((14, -18, 8)), Vector((0, 0, middle)), 0),
    }
    for name, (kind, pos, target, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = pos
        cam.rotation_euler = (0.0, 0.0, math.pi / 2) if name == "top" else (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(PREVIEW, f"{name}.png")
        bpy.ops.render.render(write_still=True)
