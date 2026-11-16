"""
Builds the Strv S, a Swedish turretless tank (the "S-tank", the Stridsvagn 103), and exports it to
client/public/models/strvs.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strvs.py -- \
        [--out client/public/models/strvs.glb] [--preview DIR]

Everything is procedural and flat-shaded, under three thousand triangles in all.

Conventions
  * Blender: Z up, the gun points along +Y, the tank's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull's bounding box (3.4 x 1.4 x 7.04 m, the real tank's size). The
    ground is 1.2 m below it.
  * The shape is the real one's: a low wedge whose long glacis is half its length, a sharp nose with
    the dozer blade folded under it, hull sides that overhang the tracks and carry a row of jerrycans as a
    skirt, four big road wheels a side with the sprocket at the front, and a commander's cupola on the right.
    There is no turret: the gun is fixed in the hull, and the whole tank turns to aim.
  * `PROFILE` is the hull's side outline the game drives on and shoots at (`PROFILE` in
    crates/shared/src/vehicles/strv.rs): the middle of the hull is drawn to it exactly.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
      `wheel_L_<n>`     the left track's four road wheels (0 to 3, front first), sprocket (4) and idler (5),
                        origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `gun`             origin on the trunnion (the game's STRV_GUN_PIVOT); the game pitches it and slides it back in recoil
  * Materials: `paint` is tinted per tank at runtime; `dark` (tracks, tyres, grilles), `metal` (barrel,
    fittings) and `steel` (lamps, shackles) keep their colours.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import MATERIALS, Part, arg, export, material, track_loops, tracked_preview

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/strvs.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with STRV_* in crates/shared/src/vehicles/strv.rs.
# --------------------------------------------------------------------------------------------------
RIDE = 1.2  # the ground below the origin
GUN_PIVOT = Vector((0.0, 1.0, 0.5))  # game [0, 0.5, -1.0]: +Z in the game is backwards
BARREL_LENGTH = 4.5
# The hull's side outline as (y, z): the game's `[z, y]` corners with the nose turned to +Y.
PROFILE = [(3.52, -0.1), (0.0, 0.7), (-1.6, 0.7), (-3.52, 0.5), (-3.52, -0.2), (-3.1, -0.7), (2.6, -0.7)]
NOSE_Y, NOSE_Z = PROFILE[0]
ROOF_FRONT_Y, ROOF_Z = PROFILE[1]
ROOF_REAR_Y = PROFILE[2][0]
TAIL_Y, TAIL_Z = PROFILE[3]
HALF_WIDTH = 1.7  # over the tracks: the collider's
BODY_HALF = 1.38  # the middle of the hull, between the sponsons
SPONSON_DROP = 0.12  # the sponsons over the tracks are this much lower than the middle
TRACK_X = 1.36  # centre line of each track
TRACK_WIDTH = 0.62
WHEEL_RADIUS = 0.4
TRACK_THICKNESS = 0.08
TRACK_Z = WHEEL_RADIUS + TRACK_THICKNESS - RIDE  # height of the road wheels' axles
ROAD_WHEELS = (1.5, 0.5, -0.5, -1.5)  # front first: the track's run on the ground is short
# The sprocket drives the track from the front, and the idler is at the back; both sit higher than the road wheels.
SPROCKET = (2.75, -0.52, 0.32)  # y, z, radius
IDLER = (-2.75, -0.55, 0.3)
TUB_HALF = TRACK_X - TRACK_WIDTH / 2  # the lower hull's side plates meet the inner faces of the tracks

GLACIS_SLOPE = (ROOF_Z - NOSE_Z) / (NOSE_Y - ROOF_FRONT_Y)  # rise per metre going back
DECK_SLOPE = (ROOF_Z - TAIL_Z) / (ROOF_REAR_Y - TAIL_Y)


def glacis_z(y):
    """Height of the middle glacis at `y`."""
    return ROOF_Z - (y - ROOF_FRONT_Y) * GLACIS_SLOPE


def deck_z(y):
    """Height of the rear deck at `y`."""
    return ROOF_Z - (ROOF_REAR_Y - y) * DECK_SLOPE


# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


MATERIALS[:] = [
    material("paint", (0.3, 0.36, 0.24)),
    material("dark", (0.1, 0.1, 0.1)),
    material("metal", (0.2, 0.22, 0.19)),
    material("steel", (0.6, 0.6, 0.56)),
]
PAINT, DARK, METAL, STEEL = range(4)


# --------------------------------------------------------------------------------------------------
# Hull
# --------------------------------------------------------------------------------------------------
hull = Part("hull")
# The lower hull between the tracks, to the game's outline: the plate under the nose leans far back to a flat belly.
hull.prism(PROFILE, -TUB_HALF, TUB_HALF, PAINT)
# The middle of the upper hull: one low wedge from the sharp nose to the tail.
hull.prism([PROFILE[0], PROFILE[1], PROFILE[2], PROFILE[3], (TAIL_Y, NOSE_Z)], -BODY_HALF, BODY_HALF, PAINT)
# The dozer blade, folded back flat against the plate under the nose, and its cutting edge.
UNDER = math.atan2(NOSE_Z - PROFILE[6][1], NOSE_Y - PROFILE[6][0])
hull.box((0.0, 3.08, -0.43), (2.0, 1.0, 0.06), METAL, rx=UNDER)
hull.box((0.0, 2.62, -0.73), (2.0, 0.08, 0.12), METAL, rx=UNDER)
# Towing shackles hanging from the nose.
for x in (-0.85, 0.85):
    hull.box((x, NOSE_Y + 0.03, NOSE_Z - 0.06), (0.12, 0.06, 0.22), STEEL)


def strip(points, x0, x1, lift, mat):
    """A band `lift` thick lying along the (y, z) line through `points`, from x0 to x1."""
    for (y0, z0), (y1, z1) in zip(points, points[1:]):
        hull.prism([(y0, z0), (y1, z1), (y1, z1 + lift), (y0, z0 + lift)], x0, x1, mat)


# A sponson's top edge: the middle's, a little lower.
SPONSON_FRONT_Y = ROOF_FRONT_Y + 0.3
SPONSON_TOP = [(NOSE_Y, NOSE_Z), (SPONSON_FRONT_Y, ROOF_Z - SPONSON_DROP), (ROOF_REAR_Y, ROOF_Z - SPONSON_DROP), (TAIL_Y, TAIL_Z - SPONSON_DROP)]
SPONSON_SLOPE = (ROOF_Z - SPONSON_DROP - NOSE_Z) / (NOSE_Y - SPONSON_FRONT_Y)


def sponson_z(y):
    """Height of a sponson's sloping front at `y`."""
    return ROOF_Z - SPONSON_DROP - (y - SPONSON_FRONT_Y) * SPONSON_SLOPE


track_outer, track_inner = track_loops([SPROCKET, *((y, TRACK_Z, WHEEL_RADIUS) for y in ROAD_WHEELS), IDLER], TRACK_THICKNESS)
for side in (-1, 1):
    # The sponson over the track, flush with the track's outer edge, and the flotation screen's lip along its top.
    x0, x1 = sorted((side * (BODY_HALF - 0.08), side * HALF_WIDTH))
    hull.prism([*SPONSON_TOP, (TAIL_Y, NOSE_Z)], x0, x1, PAINT)
    x0, x1 = sorted((side * (HALF_WIDTH - 0.15), side * (HALF_WIDTH + 0.03)))
    strip(SPONSON_TOP, x0, x1, 0.08, PAINT)
    # The track, tight round the sprocket, the road wheels and the idler.
    x = side * TRACK_X
    hull.ring(track_outer, track_inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # Nine jerrycans hang from the sponson as a skirt over the track's top run.
    for n in range(9):
        hull.box((side * (HALF_WIDTH + 0.07), 1.65 - n * 0.46, -0.3), (0.16, 0.38, 0.47), PAINT)
    # A headlamp at the nose's corner, in its guard, and the lens.
    lamp_y = 3.15
    lamp_z = sponson_z(lamp_y) + 0.13
    hull.box((side * 1.42, lamp_y, lamp_z), (0.36, 0.24, 0.28), PAINT)
    hull.cylinder((side * 1.42, lamp_y + 0.12, lamp_z), 0.1, 0.04, STEEL, axis="Y", segments=8)
    # The exhaust outlet at the sponson's tail, a mud flap behind the track and a tail lamp.
    hull.cylinder((side * 1.5, TAIL_Y - 0.06, 0.12), 0.08, 0.22, DARK, axis="Y", segments=6)
    hull.box((x, TAIL_Y - 0.01, -0.36), (TRACK_WIDTH, 0.04, 0.5), DARK)
    hull.box((side * 1.2, TAIL_Y - 0.02, 0.3), (0.14, 0.04, 0.09), STEEL)
    # A radiator grille on the rear deck, beside the magazine.
    hull.box((side * 0.72, -2.55, deck_z(-2.55) + 0.005), (0.9, 1.3, 0.03), DARK, rx=math.atan(DECK_SLOPE))
    # Ribs across the glacis, either side of the gun.
    for n in range(5):
        y = 1.45 + n * 0.42
        hull.box((side * 0.82, y, glacis_z(y) + 0.015), (0.95, 0.07, 0.05), PAINT, rx=-math.atan(GLACIS_SLOPE))
    # The travel lock's two posts on the nose: the barrel rests between them.
    hull.box((side * 0.22, 3.15, glacis_z(3.15) + 0.28), (0.08, 0.14, 0.6), METAL)

# The rolled flotation screen across the tail, and two stowage bins on the rear plate.
hull.box((0.0, TAIL_Y - 0.02, TAIL_Z - SPONSON_DROP + 0.03), (2 * HALF_WIDTH + 0.06, 0.18, 0.16), PAINT)
for x in (-0.62, 0.62):
    hull.box((x, TAIL_Y - 0.15, 0.1), (1.0, 0.3, 0.4), PAINT)
# The two fixed machine guns in their armoured box on the left sponson's front.
mg_z = sponson_z(2.45) + 0.11
hull.box((-1.42, 2.45, mg_z), (0.4, 0.95, 0.24), PAINT)
for x in (-1.5, -1.34):
    hull.cylinder((x, 2.98, mg_z + 0.02), 0.035, 0.16, DARK, axis="Y", segments=6)
# The gunner's sight in its armoured hood on the glacis, left of the gun: the weak spot the game's periscope slit is.
hull.box((-0.6, 0.38, glacis_z(0.38) + 0.06), (0.32, 0.26, 0.2), PAINT)
hull.box((-0.6, 0.515, glacis_z(0.45) + 0.08), (0.26, 0.02, 0.1), DARK)
# The driver's hatch on the left of the roof, with the rear driver's behind it.
hull.cylinder((-0.6, -0.45, ROOF_Z + 0.025), 0.34, 0.05, PAINT, segments=8)
hull.box((-0.6, -0.05, ROOF_Z + 0.05), (0.4, 0.08, 0.1), DARK)
hull.box((-0.6, -1.25, ROOF_Z + 0.02), (0.6, 0.5, 0.04), PAINT)
# The commander's cupola on the right: a ring of periscopes, the hatch lid, and a machine gun on its side.
CUPOLA = (0.65, -0.6)
hull.cylinder((*CUPOLA, ROOF_Z + 0.09), 0.42, 0.18, PAINT, segments=8)
hull.cylinder((*CUPOLA, ROOF_Z + 0.13), 0.43, 0.05, DARK, segments=8)
hull.cylinder((*CUPOLA, ROOF_Z + 0.2), 0.32, 0.04, METAL, segments=8)
hull.box((CUPOLA[0], CUPOLA[1] + 0.36, ROOF_Z + 0.22), (0.26, 0.18, 0.15), PAINT)
hull.box((CUPOLA[0] + 0.5, CUPOLA[1] + 0.05, ROOF_Z + 0.22), (0.09, 0.4, 0.11), METAL)
hull.cylinder((CUPOLA[0] + 0.5, CUPOLA[1] + 0.55, ROOF_Z + 0.23), 0.025, 0.6, METAL, axis="Y", segments=5)
# The engines' air louvres in the top of the glacis, ahead of the cupola.
for n in range(3):
    y = 0.22 + n * 0.14
    hull.box((0.68, y, glacis_z(y) + 0.012), (0.7, 0.07, 0.03), DARK, rx=-math.atan(GLACIS_SLOPE))
# Two whip aerials at the back of the roof.
for x, y in ((-1.15, -1.5), (1.15, -1.9)):
    foot = deck_z(min(y, ROOF_REAR_Y))
    hull.cylinder((x, y, foot + 0.06), 0.05, 0.12, METAL, segments=5)
    hull.cylinder((x, y, foot + 0.95), 0.012, 1.7, METAL, segments=4)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a painted dish and a bar across the hub, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    for index, y in enumerate(ROAD_WHEELS):
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, TRACK_Z))
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS, 0.42, DARK, axis="X", segments=12)
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS * 0.8, 0.48, PAINT, axis="X", segments=8)
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS * 0.3, 0.54, METAL, axis="X", segments=6)
        wheel.box((0, 0, 0), (0.51, WHEEL_RADIUS * 1.2, 0.07), METAL)
        wheel_objects.append(wheel.build())
    # The sprocket is a toothed ring, and the idler a plain small wheel.
    y, z, radius = SPROCKET
    wheel = Part(f"wheel_{label}_4", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.46, METAL, axis="X", segments=8)
    wheel.cylinder((0, 0, 0), radius * 0.55, 0.54, PAINT, axis="X", segments=8)
    for k in range(2):
        m = Matrix.Rotation(k * math.pi / 2, 4, "X") @ Matrix.Diagonal((0.5, radius * 2.1, 0.07, 1.0))
        wheel._tag(bmesh.ops.create_cube(wheel.bm, size=1.0, matrix=m)["verts"], METAL)
    wheel_objects.append(wheel.build())
    y, z, radius = IDLER
    wheel = Part(f"wheel_{label}_5", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.42, METAL, axis="X", segments=10)
    wheel.cylinder((0, 0, 0), radius * 0.6, 0.48, PAINT, axis="X", segments=8)
    wheel.box((0, 0, 0), (0.51, radius * 1.3, 0.07), METAL)
    wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# The gun: an armoured collar where it leaves the glacis, the fume extractor over the nose and a very long
# barrel past it, origin on the trunnion.
# --------------------------------------------------------------------------------------------------
gun = Part("gun", location=GUN_PIVOT)
gun.cylinder((0.0, 0.1, 0.0), 0.22, 1.2, PAINT, axis="Y", segments=8, radius2=0.17)  # collar
gun.cylinder((0.0, (0.5 + BARREL_LENGTH) / 2, 0.0), 0.12, BARREL_LENGTH - 0.5, METAL, axis="Y", segments=10, radius2=0.09)
gun.cylinder((0.0, 1.55, 0.0), 0.175, 0.85, METAL, axis="Y", segments=8)  # fume extractor
gun.cylinder((0.0, BARREL_LENGTH - 0.08, 0.0), 0.11, 0.16, METAL, axis="Y", segments=8)  # muzzle ring
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    tracked_preview(PREVIEW, 7.04, RIDE)
