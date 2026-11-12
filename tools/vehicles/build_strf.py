"""
Builds the Strf 90 (a tracked infantry fighting vehicle in the CV90's silhouette) and exports it to
client/public/models/strf.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_strf.py -- \
        [--out client/public/models/strf.glb] [--preview DIR]

Everything is procedural and flat-shaded, about two thousand triangles in all.

Conventions (as build_tank.py)
  * Blender: Z up, the gun points along +Y, the vehicle's right hand side is +X. The glTF exporter turns
    that into Y up, gun along -Z, which is the game's vehicle frame, so a game point (x, y, z) is
    Blender (x, -z, y).
  * Origin is the centre of the hull collider (a 3.0 x 1.2 x 6.4 m box, STRF_HALF_EXTENTS). The ground
    is STRF_RIDE_HEIGHT (1.01 m) below it.
  * Nodes the game looks up by name:
      `hull`            everything fixed to the hull, tracks included
      `wheel_L_<n>`     road wheels, sprocket and idler of the left track, origin on the axle: the game rolls them about X
      `wheel_R_<n>`     the same on the right
      `turret`          origin on the turret ring (the game's STRF_TURRET_PIVOT); the game yaws it
      `gun`             child of `turret`, origin on the ring too; the game pitches it and slides it back in recoil
  * Materials: `paint` is camouflaged per vehicle at runtime; `dark` (tracks, tyres, glass), `metal`
    (barrel, fittings) and `steel` (rims) keep their colours.
"""

import os
import sys

import bpy
from mathutils import Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

from common import MATERIALS, Part, arg, export, material, stadium

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/strf.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with STRF_* in crates/shared/src/vehicles/strf.rs.
# --------------------------------------------------------------------------------------------------
RIDE = 1.01  # STRF_RIDE_HEIGHT: the ground is this far below the origin
TURRET_PIVOT = Vector((0.0, 0.5, 0.9))  # game [0, 0.9, -0.5]
BARREL_LENGTH = 3.0
TRACK_X = 1.3  # STRF_TRACK_HALF: centre line of each track
TRACK_WIDTH = 0.5
WHEEL_RADIUS = 0.36  # WHEEL_RADIUS in client/src/render/strf.ts
TRACK_Z = -RIDE + WHEEL_RADIUS + 0.08  # the road wheels' axles: the track's run under them is on the ground
ROAD_WHEELS = (-2.25, -1.5, -0.75, 0.0, 0.75, 1.5, 2.25)
SPROCKET_Y, IDLER_Y = 2.9, -2.9  # driven at the front, as on the CV90
HALF_W = TRACK_X - TRACK_WIDTH / 2  # the lower hull fits between the tracks
UPPER_W = TRACK_X + TRACK_WIDTH / 2 + 0.03  # the upper hull overhangs them

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

MATERIALS[:] = [
    material("paint", (0.42, 0.48, 0.32)),
    material("dark", (0.09, 0.09, 0.09)),
    material("metal", (0.22, 0.24, 0.21)),
    material("steel", (0.5, 0.52, 0.5)),
]
PAINT, DARK, METAL, STEEL = range(4)

# --------------------------------------------------------------------------------------------------
# Hull and tracks
# --------------------------------------------------------------------------------------------------
hull = Part("hull")
# The lower hull between the tracks: a blunt wedge of a nose under the glacis, and a square tail.
hull.prism([(3.3, -0.05), (2.95, -0.58), (-2.95, -0.58), (-3.2, -0.3), (-3.2, -0.05)], -HALF_W, HALF_W, PAINT)
# The upper hull, as wide as the tracks: a long shallow glacis up to the driver's hatch, a flat roof
# back to a tall, near-vertical rear with the troop ramp in it.
hull.prism([(3.35, -0.05), (2.05, 0.6), (-3.1, 0.6), (-3.22, 0.45), (-3.22, -0.05)], -UPPER_W, UPPER_W, PAINT)
for side in (-1, 1):
    x = side * TRACK_X
    outer = stadium(SPROCKET_Y, IDLER_Y, TRACK_Z, WHEEL_RADIUS + 0.08)
    inner = stadium(SPROCKET_Y, IDLER_Y, TRACK_Z, WHEEL_RADIUS + 0.01)
    hull.ring(outer, inner, x - TRACK_WIDTH / 2, x + TRACK_WIDTH / 2, DARK)
    # Short side skirts over the top run, and a stowage bin along the roof edge.
    hull.box((side * (UPPER_W + 0.02), -0.2, -0.17), (0.04, 5.6, 0.26), PAINT)
    hull.box((side * (UPPER_W - 0.14), -2.1, 0.66), (0.26, 1.6, 0.12), PAINT)
    # A headlamp each side of the glacis and a tail light on the rear corners.
    hull.box((side * 1.15, 3.12, 0.12), (0.18, 0.12, 0.12), METAL)
    hull.box((side * 1.3, -3.25, 0.4), (0.14, 0.06, 0.1), DARK)
# The driver's hatch and periscopes at the front left, the engine grille at the front right.
hull.box((-0.75, 2.2, 0.63), (0.6, 0.6, 0.06), PAINT)
hull.box((-0.75, 2.52, 0.66), (0.4, 0.06, 0.08), DARK)
hull.box((0.62, 2.05, 0.62), (0.9, 0.75, 0.04), DARK)
# Troop roof hatches behind the turret.
for x in (-0.55, 0.55):
    hull.box((x, -1.85, 0.63), (0.7, 0.9, 0.06), PAINT)
# The rear ramp: a door panel standing proud of the tail, with its hinges at the bottom.
hull.box((0.0, -3.25, 0.12), (1.5, 0.06, 0.8), PAINT)
hull.box((0.0, -3.26, 0.42), (0.35, 0.06, 0.3), METAL)  # the door in the ramp
for x in (-0.55, 0.55):
    hull.box((x, -3.28, -0.3), (0.18, 0.08, 0.08), METAL)
# The exhaust out of the right side, behind the engine.
hull.box((UPPER_W + 0.04, 1.45, 0.35), (0.06, 0.4, 0.18), DARK)
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels: a tyre, a lighter rim and a bar across it, so that rolling can be seen.
# --------------------------------------------------------------------------------------------------
wheel_objects = []
for side, label in ((-1, "L"), (1, "R")):
    stations = [(f"{n}", y, WHEEL_RADIUS, 0.3) for n, y in enumerate(ROAD_WHEELS)]
    stations.append(("7", SPROCKET_Y, 0.3, 0.32))
    stations.append(("8", IDLER_Y, 0.32, 0.3))
    for index, y, radius, width in stations:
        z = TRACK_Z + (0.1 if index in ("7", "8") else 0.0)  # the sprocket and idler sit higher
        wheel = Part(f"wheel_{label}_{index}", location=(side * TRACK_X, y, z))
        wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=8)
        wheel.cylinder((0, 0, 0), radius * 0.6, width + 0.06, STEEL, axis="X", segments=6)
        wheel.box((0, 0, 0), (width + 0.09, radius * 1.1, 0.07), METAL)
        wheel_objects.append(wheel.build())

# --------------------------------------------------------------------------------------------------
# Turret and gun. A compact, faceted box set forward of the middle, with a bustle at the back.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=TURRET_PIVOT)
BASE = -0.3  # the hull's roof, relative to the ring centre
TOP = 0.28
# Sloped face, flat roof, a bustle that steps down at the back.
turret.prism([(1.15, BASE), (0.95, TOP), (-0.9, TOP), (-1.35, 0.1), (-1.25, BASE)], -0.88, 0.88, PAINT)
# Bevelled cheeks either side of the gun, which make the turret narrower at the top.
for side in (-1, 1):
    turret.prism([(1.15, BASE), (0.95, TOP - 0.05), (-0.6, TOP - 0.05), (-0.6, BASE)], side * 0.88, side * 1.0, PAINT)
    # A bank of smoke dischargers on each cheek, pointing forwards and out.
    turret.box((side * 0.82, 0.55, TOP + 0.06), (0.22, 0.32, 0.12), METAL)
# The gunner's sight on the right, the commander's panoramic sight and hatch on the left.
turret.box((0.48, 0.75, TOP + 0.13), (0.24, 0.3, 0.26), PAINT)
turret.box((0.48, 0.91, TOP + 0.15), (0.18, 0.03, 0.14), DARK)
turret.cylinder((-0.45, -0.35, TOP + 0.04), 0.3, 0.08, PAINT, segments=8)
turret.cylinder((-0.45, 0.25, TOP + 0.1), 0.12, 0.2, PAINT, segments=6)
turret.box((-0.45, 0.25, TOP + 0.24), (0.26, 0.26, 0.1), DARK)
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 1.2, 0.0), (0.42, 0.24, 0.34), PAINT)  # mantlet
gun.cylinder((0.0, 1.75, 0.0), 0.09, 0.9, METAL, axis="Y", segments=8)  # perforated jacket
gun.cylinder((0.0, (2.2 + BARREL_LENGTH) / 2, 0.0), 0.055, BARREL_LENGTH - 2.2, METAL, axis="Y", segments=8)
gun.cylinder((0.0, BARREL_LENGTH - 0.08, 0.0), 0.075, 0.16, DARK, axis="Y", segments=8)  # muzzle brake
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, turret_obj, gun_obj, *wheel_objects]
print("tris:", sum(len(p.vertices) - 2 for o in everything for p in o.data.polygons))
print("wrote", OUT)


# --------------------------------------------------------------------------------------------------
# Preview renders (optional)
# --------------------------------------------------------------------------------------------------
def preview(directory):
    os.makedirs(directory, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 900, 520
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "side": ("ORTHO", Vector((12, 0, -0.2)), Vector((0, 0, -0.2)), 8.0),
        "top": ("ORTHO", Vector((0, 0, 12)), Vector((0, 0, 0)), 8.0),
        "front": ("ORTHO", Vector((0, 12, 0)), Vector((0, 0, 0)), 4.5),
        "three": ("PERSP", Vector((7.5, 8.5, 4.5)), Vector((0, 0, -0.1)), 0),
        "rear": ("PERSP", Vector((-6, -9, 4.0)), Vector((0, 0, 0)), 0),
    }
    for name, (kind, pos, target, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = pos
        cam.rotation_euler = (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)


if PREVIEW:
    preview(PREVIEW)
