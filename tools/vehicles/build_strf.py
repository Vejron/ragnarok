"""
Builds the Strf 90 (a tracked infantry fighting vehicle, the Swedish CV90 with its 40 mm gun) and
exports it to client/public/models/strf.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strf.py -- \
        [--out client/public/models/strf.glb] [--preview DIR]

Everything is procedural and flat-shaded, under three thousand triangles in all.

Conventions (as build_tank.py)
  * Blender: Z up, the gun points along +Y, the vehicle's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame, so a game point (x, y, z) is
    Blender (x, -z, y).
  * Origin is the centre of the hull's bounding box (3.2 x 1.3 x 6.55 m, the real vehicle's size,
    STRF_HALF_EXTENTS). The ground is STRF_RIDE_HEIGHT (1.1 m) below it.
  * The shape is the real one's: a high nose with the engine's cover sloping gently up from it to a
    flat roof, a hull the full width over the tracks, seven small road wheels a side with the sprocket
    at the front, a faceted two-man turret forward of the middle with a thin 40 mm gun ending in a cone,
    and the troops' door in the upright rear between two stowage boxes.
  * `PROFILE` is the hull's side outline the game drives on and shoots at (`PROFILE` in
    crates/shared/src/vehicles/strf.rs): the hull is drawn to it exactly.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
      `wheel_L_<n>`     the left track's seven road wheels (0 to 6, front first), sprocket (7) and idler (8),
                        origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `turret`          origin on the turret ring (the game's STRF_TURRET_PIVOT, the middle of the turret's
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
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/strf.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with STRF_* in crates/shared/src/vehicles/strf.rs.
# --------------------------------------------------------------------------------------------------
RIDE = 1.1  # the ground below the origin
TURRET_PIVOT = Vector((0.0, 0.4, 1.025))  # game [0, 1.025, -0.4]: +Z in the game is backwards
TURRET_HALF_HEIGHT = 0.375
BARREL_LENGTH = 3.1
# The hull's side outline as (y, z): the game's `[z, y]` corners with the nose turned to +Y.
PROFILE = [(3.275, 0.2), (1.475, 0.65), (-3.275, 0.65), (-3.275, -0.3), (-3.0, -0.65), (2.6, -0.65)]
NOSE_Y, NOSE_Z = PROFILE[0]
ROOF_FRONT_Y, ROOF_Z = PROFILE[1]
TAIL_Y = PROFILE[2][0]
HALF_WIDTH = 1.6  # over the tracks: the collider's
TRACK_X = 1.33  # centre line of each track
TRACK_WIDTH = 0.5
WHEEL_RADIUS = 0.3
TRACK_THICKNESS = 0.08
TRACK_Z = WHEEL_RADIUS + TRACK_THICKNESS - RIDE  # height of the road wheels' axles
ROAD_WHEELS = (2.1, 1.4, 0.7, 0.0, -0.7, -1.4, -2.1)  # front first
# The sprocket drives the track from the front, and the idler is at the back; both sit higher than the road wheels.
SPROCKET = (2.85, -0.42, 0.3)  # y, z, radius
IDLER = (-2.85, -0.48, 0.27)
TUB_HALF = TRACK_X - TRACK_WIDTH / 2  # the lower hull's side plates meet the inner faces of the tracks
SPONSON_Z = 0.0  # the underside of the hull over the tracks

COVER = math.atan2(ROOF_Z - NOSE_Z, NOSE_Y - ROOF_FRONT_Y)  # how steeply the engine's cover climbs going back, radians


def cover_z(y):
    """Height of the sloping front at `y`."""
    return ROOF_Z - (y - ROOF_FRONT_Y) * math.tan(COVER)


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
under_nose = NOSE_Y - (NOSE_Z - SPONSON_Z) * (NOSE_Y - PROFILE[5][0]) / (NOSE_Z - PROFILE[5][1])
hull.prism([PROFILE[0], PROFILE[1], PROFILE[2], (TAIL_Y, SPONSON_Z), (under_nose, SPONSON_Z)], -HALF_WIDTH, HALF_WIDTH, PAINT)
# The engine's louvres in the right of the sloping front, and its exhaust grille in the right side.
for n in range(4):
    y = 1.85 + n * 0.3
    hull.box((0.75, y, cover_z(y) + 0.012), (1.0, 0.2, 0.03), DARK, rx=-COVER)
hull.box((HALF_WIDTH + 0.01, 1.5, 0.36), (0.03, 0.9, 0.3), DARK)
# The driver's hatch in the left of the sloping front, with its periscopes ahead of it.
hull.box((-0.8, 1.9, cover_z(1.9) + 0.03), (0.7, 0.7, 0.05), PAINT, rx=-COVER)
hull.box((-0.8, 2.3, cover_z(2.3) + 0.06), (0.5, 0.08, 0.1), DARK, rx=-COVER)
# Headlamp clusters at the nose's corners, and towing shackles under it.
for side in (-1, 1):
    hull.box((side * 1.25, 3.02, cover_z(3.02) + 0.09), (0.4, 0.2, 0.2), PAINT)
    hull.cylinder((side * 1.25, 3.13, cover_z(3.02) + 0.1), 0.075, 0.04, STEEL, axis="Y", segments=8)
    hull.box((side * 0.7, 3.12, -0.05), (0.12, 0.08, 0.22), STEEL)
# A plate of spare track links on the lower front.
UNDER = math.atan2(NOSE_Z - PROFILE[5][1], NOSE_Y - PROFILE[5][0])
hull.box((0.0, 3.0, -0.2), (1.0, 0.3, 0.06), DARK, rx=UNDER)
# The roof: the turret ring, and the troops' two long hatches behind the turret.
hull.cylinder((TURRET_PIVOT.x, TURRET_PIVOT.y, ROOF_Z + 0.02), 1.05, 0.04, METAL, segments=12)
for x in (-0.5, 0.5):
    hull.box((x, -2.0, ROOF_Z + 0.02), (0.7, 1.5, 0.04), PAINT)
    hull.box((x, -2.0, ROOF_Z + 0.045), (0.1, 0.3, 0.03), METAL)
# The rear: the troops' door with its vision block and handle, a stowage box either side and tail lamps.
hull.box((0.0, TAIL_Y - 0.03, 0.05), (1.2, 0.06, 1.05), PAINT)
hull.box((0.0, TAIL_Y - 0.07, 0.3), (0.3, 0.02, 0.1), DARK)
hull.box((0.35, TAIL_Y - 0.08, 0.0), (0.06, 0.04, 0.2), STEEL)
for side in (-1, 1):
    hull.box((side * 1.12, TAIL_Y - 0.16, 0.2), (0.8, 0.32, 0.75), PAINT)
    hull.box((side * 1.12, TAIL_Y - 0.33, -0.25), (0.16, 0.03, 0.09), STEEL)

track_outer, track_inner = track_loops([SPROCKET, *((y, TRACK_Z, WHEEL_RADIUS) for y in ROAD_WHEELS), IDLER], TRACK_THICKNESS)
for side in (-1, 1):
    x = side * TRACK_X
    hull.ring(track_outer, track_inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # A rail along the roof's edge, and a long stowage box on the side under it.
    hull.box((side * (HALF_WIDTH - 0.06), -0.95, ROOF_Z + 0.03), (0.1, 4.6, 0.06), PAINT)
    hull.box((side * (HALF_WIDTH + 0.06), -1.3, 0.36), (0.14, 2.6, 0.4), PAINT)
    # Three skirt plates over the track's top run, and a mud flap behind it.
    for n in range(3):
        hull.box((side * (HALF_WIDTH + 0.02), 2.0 - n * 2.02, -0.2), (0.04, 1.96, 0.42), PAINT)
    hull.box((x, TAIL_Y + 0.02, -0.3), (TRACK_WIDTH + 0.06, 0.04, 0.5), DARK)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a painted dish and a bar across the hub, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    for index, y in enumerate(ROAD_WHEELS):
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, TRACK_Z))
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS, 0.36, DARK, axis="X", segments=10)
        wheel.cylinder((0, 0, 0), WHEEL_RADIUS * 0.78, 0.42, PAINT, axis="X", segments=6)
        wheel.box((0, 0, 0), (0.45, WHEEL_RADIUS * 1.2, 0.06), METAL)
        wheel_objects.append(wheel.build())
    # The sprocket is a toothed ring, and the idler a plain small wheel.
    y, z, radius = SPROCKET
    wheel = Part(f"wheel_{label}_7", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.38, METAL, axis="X", segments=8)
    wheel.cylinder((0, 0, 0), radius * 0.55, 0.46, PAINT, axis="X", segments=8)
    for k in range(2):
        m = Matrix.Rotation(k * math.pi / 2, 4, "X") @ Matrix.Diagonal((0.42, radius * 2.1, 0.06, 1.0))
        wheel._tag(bmesh.ops.create_cube(wheel.bm, size=1.0, matrix=m)["verts"], METAL)
    wheel_objects.append(wheel.build())
    y, z, radius = IDLER
    wheel = Part(f"wheel_{label}_8", location=(side * TRACK_X, y, z))
    wheel.cylinder((0, 0, 0), radius, 0.36, METAL, axis="X", segments=10)
    wheel.cylinder((0, 0, 0), radius * 0.6, 0.42, PAINT, axis="X", segments=8)
    wheel.box((0, 0, 0), (0.45, radius * 1.3, 0.06), METAL)
    wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# Turret and gun. The turret is low and faceted: a sloping face, cheeks that lean in, and a short bustle.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=TURRET_PIVOT)
BASE, ROOF = -TURRET_HALF_HEIGHT, TURRET_HALF_HEIGHT - 0.03
turret.prism([(1.05, BASE), (0.92, 0.08), (0.4, ROOF), (-1.05, ROOF), (-1.4, 0.18), (-1.4, BASE)], -0.72, 0.72, PAINT)
for side in (-1, 1):
    # A cheek each side, lower than the middle, with a bank of smoke dischargers on its front.
    turret.prism([(0.9, BASE), (0.78, 0.05), (0.3, 0.2), (-1.0, 0.2), (-1.3, 0.08), (-1.3, BASE)], *sorted((side * 0.7, side * 1.0)), PAINT)
    turret.box((side * 0.92, 0.82, -0.1), (0.2, 0.14, 0.2), METAL)
# The commander's cupola on the right: a ring of periscopes and the hatch lid. The gunner's hatch on the left.
turret.cylinder((0.38, -0.3, ROOF + 0.07), 0.32, 0.14, PAINT, segments=8)
turret.cylinder((0.38, -0.3, ROOF + 0.09), 0.33, 0.04, DARK, segments=8)
turret.cylinder((0.38, -0.3, ROOF + 0.155), 0.24, 0.03, METAL, segments=8)
turret.box((-0.4, -0.3, ROOF + 0.02), (0.5, 0.55, 0.04), PAINT)
# The gunner's sight in its box on the roof's front, and the stowage rack on the bustle.
turret.box((0.45, 0.42, ROOF + 0.05), (0.24, 0.3, 0.18), PAINT)
turret.box((0.45, 0.58, ROOF + 0.06), (0.18, 0.02, 0.09), DARK)
turret.box((0.0, -1.55, -0.1), (1.5, 0.3, 0.3), METAL)
# Two whip aerials at the back of the roof.
for x in (-0.6, 0.62):
    turret.cylinder((x, -0.95, ROOF + 0.05), 0.04, 0.1, METAL, segments=5)
    turret.cylinder((x, -0.95, ROOF + 0.8), 0.012, 1.5, METAL, segments=4)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 1.0, -0.02), (0.6, 0.34, 0.46), PAINT)  # mantlet
gun.cylinder((0.0, 1.5, -0.02), 0.1, 0.8, PAINT, axis="Y", segments=8, radius2=0.075)  # the barrel's jacket
gun.cylinder((-0.22, 1.2, 0.02), 0.03, 0.2, DARK, axis="Y", segments=5)  # the coaxial machine gun
gun.cylinder((0.0, (1.15 + BARREL_LENGTH) / 2, -0.02), 0.055, BARREL_LENGTH - 1.15, METAL, axis="Y", segments=8, radius2=0.045)
gun.cylinder((0.0, BARREL_LENGTH - 0.16, -0.02), 0.05, 0.32, METAL, axis="Y", segments=8, radius2=0.11)  # the muzzle's cone
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, turret_obj, gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)

if PREVIEW:
    tracked_preview(PREVIEW, 6.55, RIDE)
