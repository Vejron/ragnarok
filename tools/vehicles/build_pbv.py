"""
Builds the Pbv 302 (the Swedish amphibious tracked troop carrier of the 1960s) and exports it to
client/public/models/pbv.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_pbv.py -- \
        [--out client/public/models/pbv.glb] [--preview DIR]

Everything is procedural and flat-shaded, under three thousand triangles in all.

Conventions (as build_strf.py)
  * Blender: Z up, the gun points along +Y, the vehicle's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame, so a game point (x, y, z) is
    Blender (x, -z, y).
  * Origin is the centre of the hull's bounding box (2.86 x 1.4 x 5.35 m, the real vehicle's size,
    PBV_HALF_EXTENTS). The ground is PBV_RIDE_HEIGHT (1.1 m) below it.
  * The shape is the real one's: a boat of a hull, the full width over the tracks, with a blunt bow:
    a short upright nose plate, a glacis leaning well back from it with the trim vane folded on it; the driver's
    hatch in the middle of the front, the gunner's small turret with its thin 20 mm gun to their left
    and the commander's hatch to their right; two long hatches over the troops, two doors in the
    upright rear, and five road wheels a side with the sprocket at the front.
  * `PROFILE` is the hull's side outline the game drives on and shoots at (`PROFILE` in
    crates/shared/src/vehicles/pbv.rs): the hull is drawn to it exactly.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
      `wheel_L_<n>`     the left track's five road wheels (0 to 4, front first), sprocket (5) and idler (6),
                        origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `turret`          origin on the turret ring (the game's PBV_TURRET_PIVOT, the middle of the turret's
                        solid); the game yaws it
      `gun`             child of `turret`, origin on the ring too; the game pitches it and slides it back in recoil
  * Materials: `paint` is camouflaged per vehicle at runtime; `dark` (tracks, tyres, glass, grilles),
    `metal` (barrel, fittings) and `steel` (lamps, shackles) keep their colours.
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
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/pbv.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with PBV_* in crates/shared/src/vehicles/pbv.rs.
# --------------------------------------------------------------------------------------------------
RIDE = 1.1  # the ground below the origin
TURRET_PIVOT = Vector((-0.75, 0.95, 0.95))  # game [-0.75, 0.95, -0.95]: +Z in the game is backwards
TURRET_HALF_HEIGHT = 0.25
BARREL_LENGTH = 2.0
# The hull's side outline as (y, z): the game's `[z, y]` corners with the nose turned to +Y.
PROFILE = [(2.675, 0.1), (1.9, 0.7), (-2.675, 0.7), (-2.675, -0.35), (-2.4, -0.7), (2.25, -0.7), (2.675, -0.2)]
NOSE_Y, NOSE_Z = PROFILE[0]
ROOF_FRONT_Y, ROOF_Z = PROFILE[1]
TAIL_Y = PROFILE[2][0]
HALF_WIDTH = 1.43  # over the tracks: the collider's
TRACK_X = 1.24  # centre line of each track
TRACK_WIDTH = 0.38
WHEEL_RADIUS = 0.33
TRACK_THICKNESS = 0.07
TRACK_Z = WHEEL_RADIUS + TRACK_THICKNESS - RIDE  # height of the road wheels' axles
ROAD_WHEELS = (1.5, 0.75, 0.0, -0.75, -1.5)  # front first
# The sprocket drives the track from the front, and the idler is at the back; both sit higher than the road wheels.
SPROCKET = (2.2, -0.42, 0.28)  # y, z, radius
IDLER = (-2.2, -0.5, 0.26)
TUB_HALF = TRACK_X - TRACK_WIDTH / 2  # the lower hull's side plates meet the inner faces of the tracks
SPONSON_Z = -0.05  # the underside of the hull over the tracks

GLACIS = math.atan2(ROOF_Z - NOSE_Z, NOSE_Y - ROOF_FRONT_Y)  # how steeply the glacis climbs going back, radians


def glacis_z(y):
    """Height of the glacis at `y`."""
    return ROOF_Z - (y - ROOF_FRONT_Y) * math.tan(GLACIS)


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
# The upper hull, the full width over the tracks: the same outline down to the sponsons' underside.
# The sponsons' underside meets the upright nose plate.
hull.prism([PROFILE[0], PROFILE[1], PROFILE[2], (TAIL_Y, SPONSON_Z), (NOSE_Y, SPONSON_Z)], -HALF_WIDTH, HALF_WIDTH, PAINT)
# The trim vane folded flat on the glacis, which is swung up before it swims, and a headlamp at each
# corner of the nose with a towing shackle under it.
hull.box((0.0, 2.33, glacis_z(2.33) + 0.03), (2.3, 0.5, 0.04), PAINT, rx=-GLACIS)
hull.box((0.0, 2.33, glacis_z(2.33) + 0.06), (0.1, 0.5, 0.03), METAL, rx=-GLACIS)
for side in (-1, 1):
    hull.box((side * 1.27, 2.55, glacis_z(2.55) + 0.1), (0.2, 0.1, 0.2), PAINT)
    hull.cylinder((side * 1.27, 2.61, glacis_z(2.55) + 0.1), 0.075, 0.04, STEEL, axis="Y", segments=8)
    hull.box((side * 0.7, NOSE_Y + 0.03, -0.05), (0.12, 0.08, 0.2), STEEL)
# The driver's hatch in the middle of the front, with three periscopes ahead of it, and the commander's
# hatch to the right with its ring of vision blocks.
hull.box((0.0, 1.4, ROOF_Z + 0.03), (0.62, 0.55, 0.06), PAINT)
for x in (-0.24, 0.0, 0.24):
    hull.box((x, 1.98, glacis_z(1.98) + 0.06), (0.18, 0.08, 0.1), DARK, rx=-GLACIS)
hull.cylinder((0.78, 0.95, ROOF_Z + 0.08), 0.32, 0.16, PAINT, segments=8)
hull.cylinder((0.78, 0.95, ROOF_Z + 0.09), 0.33, 0.05, DARK, segments=8)
hull.cylinder((0.78, 0.95, ROOF_Z + 0.17), 0.24, 0.03, METAL, segments=8)
# The turret's ring, the engine's grille in the roof behind the driver and its exhaust along the right side.
hull.cylinder((TURRET_PIVOT.x, TURRET_PIVOT.y, ROOF_Z + 0.015), 0.55, 0.03, METAL, segments=10)
hull.box((0.1, 0.3, ROOF_Z + 0.015), (0.8, 0.5, 0.03), DARK)
hull.cylinder((HALF_WIDTH + 0.07, 0.6, 0.45), 0.07, 1.0, METAL, axis="Y", segments=6)
# The troops' two long hatches, each with a handle, and three smoke dischargers at each rear corner of the roof.
for side in (-1, 1):
    hull.box((side * 0.5, -1.35, ROOF_Z + 0.02), (0.9, 2.1, 0.04), PAINT)
    hull.box((side * 0.5, -1.35, ROOF_Z + 0.045), (0.08, 0.3, 0.03), METAL)
    for n in range(3):
        hull.cylinder((side * 1.25, -2.2 - n * 0.12, ROOF_Z + 0.1), 0.04, 0.2, METAL, segments=5)
# A whip aerial at the front right of the roof.
hull.cylinder((1.25, 1.3, ROOF_Z + 0.05), 0.04, 0.1, METAL, segments=5)
hull.cylinder((1.25, 1.3, ROOF_Z + 0.8), 0.012, 1.5, METAL, segments=4)
# The rear: the two doors, each with a vision block and a handle, and tail lamps at the corners.
for side in (-1, 1):
    hull.box((side * 0.45, TAIL_Y - 0.025, 0.15), (0.84, 0.05, 0.95), PAINT)
    hull.box((side * 0.45, TAIL_Y - 0.055, 0.42), (0.22, 0.02, 0.08), DARK)
    hull.box((side * 0.1, TAIL_Y - 0.06, 0.12), (0.05, 0.04, 0.18), STEEL)
    hull.box((side * 1.2, TAIL_Y - 0.02, 0.2), (0.14, 0.03, 0.08), STEEL)

track_outer, track_inner = track_loops([SPROCKET, *((y, TRACK_Z, WHEEL_RADIUS) for y in ROAD_WHEELS), IDLER], TRACK_THICKNESS)
for side in (-1, 1):
    x = side * TRACK_X
    hull.ring(track_outer, track_inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # The hull's side: a rail along the roof's edge, the outer skin's seam, and a row of tools on it.
    hull.box((side * (HALF_WIDTH - 0.05), -0.5, ROOF_Z + 0.025), (0.08, 4.0, 0.05), PAINT)
    hull.box((side * (HALF_WIDTH + 0.015), -0.4, 0.3), (0.03, 4.4, 0.04), METAL)
    hull.box((side * (HALF_WIDTH + 0.03), -1.2 * side, 0.5), (0.05, 1.0, 0.06), METAL)
    # A skirt over the track's top run, and a mud flap behind it.
    hull.box((side * (HALF_WIDTH + 0.01), -0.3, -0.17), (0.03, 4.6, 0.26), PAINT)
    hull.box((x, TAIL_Y + 0.02, -0.3), (TRACK_WIDTH + 0.06, 0.03, 0.45), DARK)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a painted dish and a bar across the hub, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    for index, y in enumerate(ROAD_WHEELS):
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, TRACK_Z))
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS, 0.3, DARK, axis="X", segments=10)
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS * 0.78, 0.35, PAINT, axis="X", segments=8)
        wheel.box((0, 0, 0), (0.38, WHEEL_RADIUS * 1.2, 0.06), METAL)
        wheel_objects.append(wheel.build())
    # The sprocket is a toothed ring, and the idler a plain small wheel.
    y, z, radius = SPROCKET
    wheel = Part(f"wheel_{label}_{len(ROAD_WHEELS)}", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.3, METAL, axis="X", segments=8)
    wheel.cylinder((0, 0, 0), radius * 0.55, 0.36, PAINT, axis="X", segments=8)
    for k in range(2):
        m = Matrix.Rotation(k * math.pi / 2, 4, "X") @ Matrix.Diagonal((0.34, radius * 2.1, 0.06, 1.0))
        wheel._tag(bmesh.ops.create_cube(wheel.bm, size=1.0, matrix=m)["verts"], METAL)
    wheel_objects.append(wheel.build())
    y, z, radius = IDLER
    wheel = Part(f"wheel_{label}_{len(ROAD_WHEELS) + 1}", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.3, METAL, axis="X", segments=10)
    wheel.cylinder((0, 0, 0), radius * 0.6, 0.35, PAINT, axis="X", segments=8)
    wheel.box((0, 0, 0), (0.38, radius * 1.3, 0.06), METAL)
    wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# Turret and gun. The turret is a small drum that leans in to a flat roof, with a hatch, a ring of vision
# blocks and the sight's head; the gun is a thin barrel out of a boxed cradle in its face.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=TURRET_PIVOT)
BASE, ROOF = -TURRET_HALF_HEIGHT, TURRET_HALF_HEIGHT - 0.03
turret.cylinder((0, 0, (BASE + ROOF) / 2), 0.5, ROOF - BASE, PAINT, segments=10, radius2=0.4)
turret.cylinder((0, 0, ROOF - 0.08), 0.43, 0.06, DARK, segments=10)
turret.box((0.0, -0.12, ROOF + 0.02), (0.5, 0.42, 0.04), PAINT)  # the hatch's lid
turret.box((0.2, 0.26, ROOF + 0.05), (0.14, 0.14, 0.12), PAINT)  # the sight's head
turret.box((0.2, 0.335, ROOF + 0.06), (0.1, 0.02, 0.07), DARK)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 0.42, 0.0), (0.26, 0.4, 0.24), PAINT)  # the cradle in the turret's face
gun.box((-0.19, 0.3, 0.0), (0.12, 0.3, 0.2), METAL)  # the magazine on its left
gun.cylinder((0.0, 0.85, 0.0), 0.06, 0.5, METAL, axis="Y", segments=6)  # the recoil spring's jacket
gun.cylinder((0.0, (1.1 + BARREL_LENGTH) / 2, 0.0), 0.035, BARREL_LENGTH - 1.1, METAL, axis="Y", segments=6, radius2=0.03)
gun.cylinder((0.0, BARREL_LENGTH - 0.09, 0.0), 0.05, 0.18, METAL, axis="Y", segments=6)  # the muzzle brake
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, turret_obj, gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    tracked_preview(PREVIEW, 5.35, RIDE)
