"""
Builds the tank (a Strv 104, the Swedish Centurion) and exports it to client/public/models/tank.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_tank.py -- \
        [--out client/public/models/tank.glb] [--preview DIR]

Everything is procedural and flat-shaded, under three thousand triangles in all.

Conventions
  * Blender: Z up, the gun points along +Y, the tank's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull's bounding box (3.4 x 1.25 x 7.6 m, the real tank's size). The
    ground is 1.13 m below it.
  * The shape is the real one's: a long hull with a short steep glacis, track guards carrying stowage
    bins, skirt plates over the top of six road wheels in pairs, the sprocket at the back under a long
    engine deck, and a tall slab-sided turret with bins on its sides, a basket behind and a long gun with
    a fume extractor half way along.
  * `PROFILE` is the hull's side outline the game drives on and shoots at (`PROFILE` in
    crates/shared/src/vehicles/tank.rs): the middle of the hull is drawn to it exactly.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
      `wheel_L_<n>`     the left track's six road wheels (0 to 5, front first), sprocket (6) and idler (7),
                        origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `turret`          origin on the turret ring (the game's TANK_TURRET_PIVOT, the middle of the turret's
                        solid); the game yaws it
      `gun`             child of `turret`, origin on the ring too; the game pitches it and slides it back in recoil
      `mg`              child of `turret`, origin where the roof gun turns (the game's TANK_MG_AT); the game yaws it
      `mg_gun`          child of `mg`, same origin; the game pitches it
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
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/tank.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with TANK_* in crates/shared/src/vehicles/tank.rs.
# --------------------------------------------------------------------------------------------------
RIDE = 1.13  # the ground below the origin
TURRET_PIVOT = Vector((0.0, 0.4, 1.125))  # game [0, 1.125, -0.4]: +Z in the game is backwards
TURRET_HALF_HEIGHT = 0.5
BARREL_LENGTH = 5.7
MG_AT = Vector((0.5, -0.35, 0.75))  # game turret frame [0.5, 0.75, 0.35]
MG_LENGTH = 1.1
# The hull's side outline as (y, z): the game's `[z, y]` corners with the nose turned to +Y.
PROFILE = [(3.8, -0.1), (2.68, 0.625), (-3.5, 0.625), (-3.8, 0.2), (-3.8, -0.2), (-3.45, -0.625), (3.35, -0.625)]
NOSE_Y, NOSE_Z = PROFILE[0]
GLACIS_TOP_Y, ROOF_Z = PROFILE[1]
TAIL_Y = PROFILE[3][0]
HALF_WIDTH = 1.7  # over the tracks: the collider's
BODY_HALF = 1.3  # the upper hull, between the track guards
TRACK_X = 1.4  # centre line of each track
TRACK_WIDTH = 0.6
WHEEL_RADIUS = 0.38
TRACK_THICKNESS = 0.08
TRACK_Z = WHEEL_RADIUS + TRACK_THICKNESS - RIDE  # height of the road wheels' axles
ROAD_WHEELS = (2.55, 1.7, 0.6, -0.25, -1.3, -2.15)  # front first, in three pairs
# The idler is at the front and the sprocket drives from the back; both sit higher than the road wheels.
IDLER = (3.2, -0.5, 0.3)  # y, z, radius
SPROCKET = (-3.2, -0.52, 0.33)
TUB_HALF = TRACK_X - TRACK_WIDTH / 2  # the lower hull's side plates meet the inner faces of the tracks
GUARD_Z = 0.12  # the track guards' top

GLACIS = math.atan2(ROOF_Z - NOSE_Z, NOSE_Y - GLACIS_TOP_Y)  # how steeply the glacis climbs going back, radians


def glacis_z(y):
    """Height of the glacis at `y`."""
    return NOSE_Z + (NOSE_Y - y) * math.tan(GLACIS)


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
# The lower hull between the tracks, to the game's outline.
hull.prism(PROFILE, -TUB_HALF, TUB_HALF, PAINT)
# The upper hull: the glacis, the long flat roof and the bevel down to the tail.
hull.prism([PROFILE[0], PROFILE[1], PROFILE[2], PROFILE[3], (TAIL_Y, NOSE_Z)], -BODY_HALF, BODY_HALF, PAINT)
# On the glacis: a rail of spare track links, two headlamps in their guards and the driver's visor block.
hull.box((0.2, 3.5, glacis_z(3.5) + 0.03), (1.5, 0.16, 0.08), DARK, rx=-GLACIS)
for x in (-0.25, 0.95):
    hull.box((x, 3.05, glacis_z(3.05) + 0.1), (0.26, 0.2, 0.24), PAINT)
    hull.cylinder((x, 3.16, glacis_z(3.05) + 0.11), 0.09, 0.04, STEEL, axis="Y", segments=8)
hull.box((-0.7, 3.12, glacis_z(3.12) + 0.04), (0.42, 0.16, 0.14), PAINT)
hull.box((-0.7, 3.21, glacis_z(3.12) + 0.05), (0.32, 0.02, 0.07), DARK)
# Towing shackles on the nose.
for x in (-0.8, 0.8):
    hull.box((x, NOSE_Y + 0.02, NOSE_Z - 0.08), (0.12, 0.06, 0.22), STEEL)
# The driver's two hatch lids at the front left of the roof.
for x in (-0.95, -0.45):
    hull.box((x, 2.25, ROOF_Z + 0.02), (0.44, 0.6, 0.04), PAINT)
hull.box((-0.7, 2.58, ROOF_Z + 0.05), (0.5, 0.06, 0.08), DARK)
# The turret ring.
hull.cylinder((TURRET_PIVOT.x, TURRET_PIVOT.y, ROOF_Z + 0.02), 1.2, 0.04, METAL, segments=12)
# The engine deck: a raised cover with four louvred panels, and the transmission's cover behind them.
hull.box((0.0, -2.3, ROOF_Z + 0.03), (2.3, 2.3, 0.06), PAINT)
for x in (-0.58, 0.58):
    for y in (-1.7, -2.75):
        hull.box((x, y, ROOF_Z + 0.07), (0.95, 0.85, 0.03), DARK)
# The rear plate: an infantry telephone box, a towing hook and tail lamps.
hull.box((0.75, TAIL_Y - 0.08, 0.05), (0.4, 0.16, 0.3), PAINT)
hull.box((0.0, TAIL_Y - 0.05, -0.15), (0.2, 0.14, 0.14), STEEL)
for x in (-1.1, 1.1):
    hull.box((x, TAIL_Y - 0.02, 0.1), (0.14, 0.04, 0.09), STEEL)

track_outer, track_inner = track_loops([IDLER, *((y, TRACK_Z, WHEEL_RADIUS) for y in ROAD_WHEELS), SPROCKET], TRACK_THICKNESS)
for side in (-1, 1):
    x = side * TRACK_X
    hull.ring(track_outer, track_inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # The track guard the length of the hull, turned down at the front, with a mud flap at the back.
    guard_x = side * (BODY_HALF + HALF_WIDTH + 0.04) / 2
    guard_w = HALF_WIDTH + 0.04 - BODY_HALF + 0.1
    hull.box((guard_x, -0.15, GUARD_Z - 0.03), (guard_w, 7.1, 0.06), PAINT)
    hull.box((guard_x, 3.58, GUARD_Z - 0.13), (guard_w, 0.45, 0.05), PAINT, rx=-0.5)
    hull.box((x, TAIL_Y + 0.08, -0.2), (TRACK_WIDTH + 0.06, 0.04, 0.6), DARK)
    # Three stowage bins along the guard.
    for y, length in ((1.75, 1.5), (0.05, 1.7), (-1.55, 1.3)):
        hull.box((side * 1.5, y, GUARD_Z + 0.19), (0.38, length, 0.38), PAINT)
    # The exhaust in its cover on the guard beside the engine, and its pipe.
    hull.box((side * 1.5, -2.85, GUARD_Z + 0.13), (0.34, 1.1, 0.26), METAL)
    hull.cylinder((side * 1.5, -3.5, GUARD_Z + 0.13), 0.07, 0.24, DARK, axis="Y", segments=6)
    # Two long skirt plates over the top of the wheels.
    for y in (1.6, -1.55):
        hull.box((side * (HALF_WIDTH + 0.05), y, -0.22), (0.04, 3.05, 0.64), PAINT)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a painted dish and a bar across the hub, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    for index, y in enumerate(ROAD_WHEELS):
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, TRACK_Z))
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS, 0.42, DARK, axis="X", segments=10)
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS * 0.8, 0.48, PAINT, axis="X", segments=6)
        wheel.box((0, 0, 0), (0.51, WHEEL_RADIUS * 1.2, 0.07), METAL)
        wheel_objects.append(wheel.build())
    # The sprocket is a toothed ring, and the idler a plain small wheel.
    y, z, radius = SPROCKET
    wheel = Part(f"wheel_{label}_6", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.44, METAL, axis="X", segments=8)
    wheel.cylinder((0, 0, 0), radius * 0.55, 0.52, PAINT, axis="X", segments=8)
    for k in range(2):
        m = Matrix.Rotation(k * math.pi / 2, 4, "X") @ Matrix.Diagonal((0.48, radius * 2.1, 0.07, 1.0))
        wheel._tag(bmesh.ops.create_cube(wheel.bm, size=1.0, matrix=m)["verts"], METAL)
    wheel_objects.append(wheel.build())
    y, z, radius = IDLER
    wheel = Part(f"wheel_{label}_7", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.42, METAL, axis="X", segments=10)
    wheel.cylinder((0, 0, 0), radius * 0.6, 0.48, PAINT, axis="X", segments=8)
    wheel.box((0, 0, 0), (0.51, radius * 1.3, 0.07), METAL)
    wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# Turret and gun. The turret is tall and slab-sided, with a flat face for the mantlet and a bustle behind.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=TURRET_PIVOT)
BASE, ROOF = -TURRET_HALF_HEIGHT, TURRET_HALF_HEIGHT - 0.03
turret.prism([(1.12, BASE), (1.0, 0.3), (0.55, ROOF), (-1.25, ROOF), (-1.75, 0.3), (-1.75, -0.22), (-1.2, BASE)], -0.95, 0.95, PAINT)
for side in (-1, 1):
    # A stowage bin along each side, and the smoke dischargers ahead of it.
    turret.box((side * 1.08, -0.45, -0.05), (0.3, 1.9, 0.5), PAINT)
    turret.box((side * 1.03, 0.78, 0.0), (0.16, 0.24, 0.22), METAL)
# The stowage basket behind the bustle.
turret.box((0.0, -1.95, -0.02), (1.6, 0.4, 0.34), METAL)
# The commander's cupola on the right, behind the roof gun, and its hatch lid.
turret.cylinder((0.5, -0.85, ROOF + 0.09), 0.38, 0.18, PAINT, segments=8)
turret.cylinder((0.5, -0.85, ROOF + 0.2), 0.28, 0.04, METAL, segments=8)
# The loader's hatch on the left, and the gunner's sight on the roof's front.
turret.box((-0.5, -0.35, ROOF + 0.02), (0.55, 0.6, 0.04), PAINT)
turret.box((0.45, 0.72, ROOF + 0.0), (0.2, 0.2, 0.14), PAINT)
# Two whip aerials at the back of the roof.
for x in (-0.8, 0.85):
    turret.cylinder((x, -1.45, ROOF + 0.05), 0.045, 0.1, METAL, segments=5)
    turret.cylinder((x, -1.45, ROOF + 0.85), 0.012, 1.6, METAL, segments=4)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 1.14, 0.02), (1.0, 0.3, 0.62), PAINT)  # mantlet
gun.cylinder((0.0, 1.45, 0.02), 0.2, 0.5, PAINT, axis="Y", segments=8, radius2=0.15)  # the barrel's sleeve
gun.cylinder((0.0, (1.25 + BARREL_LENGTH) / 2, 0.02), 0.115, BARREL_LENGTH - 1.25, METAL, axis="Y", segments=10, radius2=0.09)
gun.cylinder((0.0, 3.6, 0.02), 0.17, 0.8, METAL, axis="Y", segments=8)  # fume extractor
gun.cylinder((0.0, BARREL_LENGTH - 0.07, 0.02), 0.105, 0.14, METAL, axis="Y", segments=8)  # muzzle ring
gun_obj = gun.build()

# The roof machine gun on its pintle ahead of the cupola: the post turns, the gun on it nods.
mg = Part("mg", location=MG_AT, parent=turret_obj)
mg.cylinder((0.0, 0.0, -0.14), 0.045, 0.28, METAL, segments=6)
mg.box((0.0, 0.0, -0.01), (0.14, 0.14, 0.05), METAL)
mg_obj = mg.build()

mg_gun = Part("mg_gun", parent=mg_obj)
mg_gun.box((0.0, 0.08, 0.05), (0.11, 0.62, 0.12), DARK)  # receiver
mg_gun.box((-0.14, 0.08, 0.04), (0.17, 0.28, 0.2), METAL)  # ammunition box
mg_gun.box((0.0, -0.27, 0.04), (0.2, 0.05, 0.05), DARK)  # spade grips
mg_gun.cylinder((0.0, (0.38 + MG_LENGTH) / 2, 0.05), 0.034, MG_LENGTH - 0.38, DARK, axis="Y", segments=6)
mg_gun.cylinder((0.0, MG_LENGTH - 0.05, 0.05), 0.055, 0.1, METAL, axis="Y", segments=6)  # muzzle brake
mg_gun_obj = mg_gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, turret_obj, gun_obj, mg_obj, mg_gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    tracked_preview(PREVIEW, 7.6, RIDE)
