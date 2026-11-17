"""
Builds the Stridsbåt 90 (the Swedish fast assault craft) and exports it to client/public/models/cb90.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_cb90.py -- \
        [--out client/public/models/cb90.glb] [--preview DIR]

Everything is procedural and flat-shaded, under two and a half thousand triangles.

Conventions (as build_pbv.py)
  * Blender: Z up, the bow points along +Y, the boat's right hand side is +X. The glTF exporter turns that
    into Y up, bow along -Z, which is the game's vehicle frame, so a game point (x, y, z) is Blender
    (x, -z, y).
  * Origin is the centre of the hull's bounding box (3.8 x 2.0 x 15.9 m, the real boat's size,
    CB90_HALF_EXTENTS). The waterline at rest is CB90_DRAFT (0.8 m) over the keel, 0.2 m below it.
  * The shape is the real one's, from bow to stern:
      - a deep-V planing hull with flared sides, a fine pointed bow and a raked stem, a rubbing strake
        along its sheer, fenders hung on its sides and two waterjet nozzles in the transom;
      - the low cockpit well forward, its front raked, three windows across it and a window band round
        its sides, with the twin machine guns fixed on its roof's front edge and the windscreen's frame
        bars reaching forward over the fore deck;
      - behind it the gunner's open ring with its railing and the cannon on a pedestal, a mast either
        side;
      - the troop compartment down in the hull, portholes in the hull's sides and its roof a little over
        the gunwale, with rails along it;
      - the open after deck with the engines' hatch, a bench either side for the riders, a rail round it
        all and a mast.
  * Nodes the game looks up by name:
      `hull`     everything fixed to the hull
      `turret`   the cannon's pedestal and cradle, origin on its pivot (CB90_CANNON_PIVOT); the game yaws it
      `gun`      child of `turret`, origin on the pivot too; the game pitches it and slides it back in recoil
      `mg`       the machine guns' fixed cradle, origin on their pivot (CB90_MG_PIVOT)
      `mg_gun`   child of `mg`, origin on the pivot too, the two barrels either side; the game pitches it
  * Materials: `paint` is camouflaged per boat at runtime; `dark` (glass, fenders, nozzles), `metal`
    (guns, rails, fittings), `steel` (lamps) and `buoy` (life rings) keep their colours.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import MATERIALS, Part, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/cb90.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with CB90_* in crates/shared/src/vehicles/cb90.rs; Blender y is the
# game's -z.
# --------------------------------------------------------------------------------------------------
HALF_BEAM, HALF_HEIGHT, HALF_LENGTH = 1.9, 1.0, 7.95
BOW = 4.5  # the sides taper to the stem over about this much (CB90_BOW)
DECK = HALF_HEIGHT
KEEL = -HALF_HEIGHT
WATERLINE = KEEL + 0.8
# The cockpit's side outline as (y, z), bow first: its raked front, its roof 1.15 m over the deck and its
# upright back. Its sides lean in from COCKPIT_BASE_W to COCKPIT_TOP_W, and its front narrows to follow
# the bow.
COCKPIT_ROOF = 2.15
COCKPIT = [(4.75, 0.95), (4.45, 1.5), (4.15, COCKPIT_ROOF), (1.7, COCKPIT_ROOF), (1.7, 0.95)]
COCKPIT_BASE_W, COCKPIT_TOP_W = 1.45, 1.2
# The troop compartment's roof, from behind the cockpit to the after deck (game CB90_TROOP_*).
TROOP_ROOF = 1.35
TROOP_FRONT, TROOP_BACK = 1.7, -2.6
TROOP_HALF_W = 1.6
RING = Vector((0.0, 0.9, 1.55))  # game CB90_RING_AT [0, 1.55, -0.9]
RING_RADIUS, RING_HALF_HEIGHT = 0.75, 0.2
CANNON_PIVOT = Vector((0.0, 0.9, 2.75))  # game [0, 2.75, -0.9]
CANNON_LENGTH = 2.3
MG_PIVOT = Vector((0.0, 3.9, 2.4))  # game [0, 2.4, -3.9]
MG_BARREL_X = 0.16
MG_LENGTH = 1.1
RAIL = 0.8  # how high the rails stand over what they stand on

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
    """The deck's height at `y`: level aft, climbing a little towards the stem."""
    s = max(0.0, (y - 1.0) / (HALF_LENGTH - 1.0))
    return DECK + 0.3 * s**1.6


def half_beam(y):
    """Half the hull's width at its deck edge at `y`: full aft, narrowing smoothly to the stem."""
    return HALF_BEAM * (1.0 - 0.95 * max(0.0, y / HALF_LENGTH) ** 2.2)


def section(y):
    """The hull's cross-section at `y` as (x, z) corners from port deck edge round under the keel to
    starboard: deck edge, chine, keel, chine, deck edge. The sides flare out above the chine, more
    forward, and the forefoot rises into a raked stem."""
    t = max(0.0, (y - (HALF_LENGTH - BOW)) / BOW)  # 0 abaft the bow, 1 at the stem
    k = max(0.0, (y - 2.0) / (HALF_LENGTH - 2.0))
    beam = half_beam(y)
    keel = KEEL + 1.4 * k**1.8
    deck = sheer(y)
    chine = min(keel + 0.4 + 0.3 * t, deck - 0.2)
    flare = 0.88 - 0.25 * t
    return [(-beam, deck), (-beam * flare, chine), (0.0, keel), (beam * flare, chine), (beam, deck)]


def cockpit_w(z, y, pad=0.0):
    """Half the cockpit's width at height `z` and `y`: its sides lean in and its front narrows."""
    f = (z - 0.95) / (COCKPIT_ROOF - 0.95)
    narrow = 1.0 - 0.2 * max(0.0, (y - 2.5) / (COCKPIT[0][0] - 2.5))
    return (COCKPIT_BASE_W + (COCKPIT_TOP_W - COCKPIT_BASE_W) * f) * narrow + pad


def leaning(part, profile, mat, pad=0.0):
    """Extrudes a closed (y, z) outline across X between the cockpit's leaning sides."""
    a = [part.bm.verts.new((-cockpit_w(z, y, pad), y, z)) for y, z in profile]
    b = [part.bm.verts.new((cockpit_w(z, y, pad), y, z)) for y, z in profile]
    sides = [part.bm.faces.new(a), part.bm.faces.new(b[::-1])]
    faces = list(sides)
    for i in range(len(profile)):
        j = (i + 1) % len(profile)
        faces.append(part.bm.faces.new((a[i], a[j], b[j], b[i])))
    for face in faces:
        face.material_index = mat
    bmesh.ops.triangulate(part.bm, faces=sides)


def bar(part, p0, p1, radius, mat, segments=4):
    """A round bar from `p0` to `p1`: a rail, a post, a frame's tube."""
    p0, p1 = Vector(p0), Vector(p1)
    along = p1 - p0
    m = Matrix.Translation((p0 + p1) / 2) @ along.to_track_quat("Z", "Y").to_matrix().to_4x4()
    verts = bmesh.ops.create_cone(part.bm, cap_ends=True, cap_tris=False, segments=segments, radius1=radius, radius2=radius, depth=along.length, matrix=m)["verts"]
    part._tag(verts, mat)


def rail(part, points, height, mat=METAL, posts=True):
    """A rail along `points` (x, y, z where it stands), `height` over them, with a post at each point."""
    tops = [Vector(p) + Vector((0, 0, height)) for p in points]
    for a, b in zip(tops, tops[1:]):
        bar(part, a, b, 0.03, mat)
    if posts:
        for p, top in zip(points, tops):
            bar(part, p, top, 0.025, mat)


def life_ring(part, centre, facing_x):
    """A life ring hung upright, facing along X (`facing_x` 1) or along Y (0)."""
    axis = "X" if facing_x else "Y"
    part.cylinder(centre, 0.3, 0.08, BUOY, axis=axis, segments=10)
    offset = Vector((0.045, 0, 0)) if facing_x else Vector((0, 0.045, 0))
    for sign in (-1, 1):
        part.cylinder(Vector(centre) + sign * offset, 0.17, 0.01, PAINT, axis=axis, segments=8)


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

for side in (-1, 1):
    # The rubbing strake along the sheer, abaft the bow.
    hull.box((side * (HALF_BEAM + 0.05), (0.5 - HALF_LENGTH) / 2, DECK - 0.15), (0.12, 0.5 + HALF_LENGTH, 0.2), DARK)
    # The waterjet nozzles in the transom, with their reversing buckets over them.
    hull.cylinder((side * 0.8, -HALF_LENGTH - 0.2, WATERLINE - 0.25), 0.28, 0.4, DARK, axis="Y", segments=8)
    hull.box((side * 0.8, -HALF_LENGTH - 0.42, WATERLINE + 0.05), (0.62, 0.12, 0.3), METAL)
    # The troop compartment's portholes in the hull's flared side, and two fenders hung over it.
    for y in (-1.6, -0.5, 0.6):
        hull.box((side * 1.84, y, 0.55), (0.06, 0.6, 0.16), DARK)
    for y in (1.2, -2.2):
        hull.cylinder((side * (HALF_BEAM + 0.2), y, 0.55), 0.17, 0.75, DARK, segments=8)

# --------------------------------------------------------------------------------------------------
# The cockpit: low and forward, three windows across its raked front and a band round its sides.
# --------------------------------------------------------------------------------------------------
leaning(hull, COCKPIT, PAINT)
leaning(hull, [(4.0, 1.65), (4.0, 1.98), (2.0, 1.98), (2.0, 1.65)], DARK, pad=0.02)
front = Vector(COCKPIT[2]) - Vector(COCKPIT[1])
for x in (-0.75, 0.0, 0.75):
    low = Vector(COCKPIT[1]) + front * 0.2
    high = Vector(COCKPIT[1]) + front * 0.85
    normal = Vector((front.y, -front.x)).normalized() * 0.03
    hull.prism([tuple(low + normal), tuple(high + normal), tuple(high - normal), tuple(low - normal)], x - 0.32, x + 0.32, DARK)
# The windscreen's frame: two bars from the roof's front corners reaching forward and down to the fore
# deck, and a searchlight on the roof.
for side in (-1, 1):
    top = (side * 1.0, 4.1, COCKPIT_ROOF + 0.05)
    bar(hull, top, (side * 1.1, 5.4, COCKPIT_ROOF + 0.35), 0.035, METAL)
    bar(hull, (side * 1.1, 5.4, COCKPIT_ROOF + 0.35), (side * 0.8, 6.6, sheer(6.6)), 0.035, METAL)
hull.cylinder((-0.9, 2.2, COCKPIT_ROOF + 0.15), 0.12, 0.22, STEEL, axis="Y", segments=8)
# The machine guns' footing on the roof's front edge.
hull.box((MG_PIVOT.x, MG_PIVOT.y, (COCKPIT_ROOF + MG_PIVOT.z) / 2 - 0.05), (0.5, 0.35, MG_PIVOT.z - COCKPIT_ROOF - 0.1), METAL)

# --------------------------------------------------------------------------------------------------
# The troop compartment's roof, the gunner's ring with its railing, and the masts.
# --------------------------------------------------------------------------------------------------
hull.box((0.0, (TROOP_FRONT + TROOP_BACK) / 2, (DECK + TROOP_ROOF) / 2 - 0.02), (2 * TROOP_HALF_W, TROOP_FRONT - TROOP_BACK, TROOP_ROOF - DECK + 0.04), PAINT)
for k in range(3):
    hull.box((0.55, -0.4 - k * 0.45, TROOP_ROOF + 0.02), (0.6, 0.3, 0.04), DARK)  # vents
hull.box((0.0, -1.8, TROOP_ROOF + 0.03), (1.1, 0.9, 0.06), PAINT)  # the roof hatch
for side in (-1, 1):
    x = side * (TROOP_HALF_W - 0.1)
    rail(hull, [(x, y, TROOP_ROOF) for y in (0.2, -0.8, -1.7, TROOP_BACK + 0.1)], RAIL)
    bar(hull, (x, 0.2, TROOP_ROOF + RAIL / 2), (x, TROOP_BACK + 0.1, TROOP_ROOF + RAIL / 2), 0.025, METAL)
hull.cylinder(RING, RING_RADIUS, 2 * RING_HALF_HEIGHT, PAINT, segments=10)
hull.cylinder(RING + Vector((0, 0, RING_HALF_HEIGHT)), RING_RADIUS - 0.12, 0.02, DARK, segments=10)
around = [RING + Vector((0.95 * math.sin(a), 0.95 * math.cos(a), RING_HALF_HEIGHT - 0.2)) for a in [k * math.pi / 4 for k in range(9)]]
rail(hull, [tuple(p) for p in around], 0.75)
life_ring(hull, (0.0, RING.y + 1.0, RING.z + 0.35), facing_x=False)
for side in (-1, 1):
    base = (side * 1.3, 1.45, TROOP_ROOF)
    bar(hull, base, (side * 1.3, 1.45, 4.0), 0.04, METAL)
    hull.box((side * 1.3, 1.45, 3.4), (0.2, 0.15, 0.25), STEEL)
bar(hull, (1.3, -2.4, TROOP_ROOF), (1.3, -2.4, 4.6), 0.05, METAL)
hull.box((1.3, -2.4, 3.6), (0.9, 0.08, 0.08), METAL)
hull.box((1.3, -2.4, 4.0), (0.2, 0.2, 0.3), STEEL)

# --------------------------------------------------------------------------------------------------
# The decks: the fore deck's ramp and bollards; on the after deck the engines' hatch, a bench either
# side for the riders, a rail round it and a life ring on it.
# --------------------------------------------------------------------------------------------------
hull.box((0.0, 5.6, sheer(5.6) + 0.03), (1.1, 1.3, 0.06), PAINT)
hull.box((0.0, 4.95, sheer(4.95) + 0.06), (1.2, 0.12, 0.1), METAL)
aft_back = -HALF_LENGTH + 0.15
hull.box((0.0, -6.0, DECK + 0.03), (1.6, 2.6, 0.06), PAINT)
for side in (-1, 1):
    hull.box((side * 0.9, -4.6, DECK + 0.22), (0.5, 1.4, 0.44), PAINT)
    x = side * (HALF_BEAM - 0.12)
    rail(hull, [(x, y, DECK) for y in (TROOP_BACK, -4.4, -6.2, aft_back)], RAIL)
    bar(hull, (x, TROOP_BACK, DECK + RAIL / 2), (x, aft_back, DECK + RAIL / 2), 0.025, METAL)
    for y in (6.3, -HALF_LENGTH + 0.6):
        hull.cylinder((side * 0.6 if y > 0 else side * 1.3, y, sheer(y) + 0.12), 0.09, 0.24, METAL, segments=6)
rail(hull, [(-HALF_BEAM + 0.12, aft_back, DECK), (HALF_BEAM - 0.12, aft_back, DECK)], RAIL, posts=False)
life_ring(hull, (HALF_BEAM - 0.08, -5.3, DECK + 0.45), facing_x=True)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# The cannon: a pedestal up from the ring's floor with the cradle on it, which the game turns; and a long
# thin barrel with an ammunition box beside its receiver, which it elevates.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=CANNON_PIVOT)
floor = RING.z + RING_HALF_HEIGHT - CANNON_PIVOT.z
turret.cylinder((0, 0, (floor - 0.2) / 2), 0.12, -floor - 0.2, METAL, segments=8)
turret.box((0.0, 0.0, -0.15), (0.5, 0.45, 0.12), METAL)
turret.box((0.3, -0.05, 0.0), (0.12, 0.3, 0.3), METAL)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 0.05, 0.0), (0.22, 0.9, 0.24), METAL)
gun.box((-0.22, -0.05, -0.02), (0.2, 0.35, 0.28), PAINT)  # the ammunition box
gun.cylinder((0.0, 0.7, 0.0), 0.07, 0.4, METAL, axis="Y", segments=6)
gun.cylinder((0.0, (0.9 + CANNON_LENGTH) / 2, 0.0), 0.04, CANNON_LENGTH - 0.9, METAL, axis="Y", segments=6, radius2=0.035)
gun.cylinder((0.0, CANNON_LENGTH - 0.1, 0.0), 0.055, 0.2, METAL, axis="Y", segments=6)
gun.box((0.0, -0.55, 0.05), (0.3, 0.06, 0.12), METAL)  # the handles
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# The machine guns: a fixed cradle on the cockpit's roof, and two Ksp 58s side by side with their
# ammunition boxes, which the game tilts.
# --------------------------------------------------------------------------------------------------
mg = Part("mg", location=MG_PIVOT)
mg.box((0.0, 0.0, -0.08), (0.55, 0.3, 0.06), METAL)
mg_obj = mg.build()

mg_gun = Part("mg_gun", parent=mg_obj)
for side in (-1, 1):
    x = side * MG_BARREL_X
    mg_gun.box((x, 0.0, 0.0), (0.09, 0.7, 0.14), METAL)
    mg_gun.cylinder((x, (0.35 + MG_LENGTH) / 2, 0.0), 0.022, MG_LENGTH - 0.35, METAL, axis="Y", segments=5)
    mg_gun.cylinder((x, MG_LENGTH - 0.05, 0.0), 0.035, 0.1, METAL, axis="Y", segments=5)
    mg_gun.box((x + side * 0.12, -0.05, -0.05), (0.14, 0.25, 0.18), PAINT)
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
