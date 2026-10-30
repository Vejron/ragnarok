"""
Builds the three cars, an open jeep, a low sports car and the thruster Roadster, and exports them
to client/public/models/.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- \
        [--out DIR] [--only jeep|sports|roadster] [--preview DIR]

Everything is procedural and flat-shaded: about a thousand triangles each.

Conventions
  * Blender: Z up, the nose points along +Y, the car's right hand side is +X. The glTF exporter turns
    that into Y up, nose along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the chassis collider. Dimensions below are kept in step with JEEP_* and
    SPORTS_* in crates/shared/src/vehicles/ (and client/src/sim/consts.ts).
  * Nodes the game looks up by name:
      `pivot_FL`, `pivot_FR`, `pivot_RL`, `pivot_RR`  empties on each wheel's hub; the game steers the front ones about Y
      `wheel_FL` ... `wheel_RR`                        the wheels, children of the pivots, origin on the axle: the game rolls them about X
      `seat_0` ... `seat_N`                            empties where a seated player's head is drawn, driver first
      `nozzle_rear_N`, `nozzle_lift_N`                the Roadster's thruster mouths: the rear ones blow backwards, the lift ones down
  * Materials: `paint` is tinted per car at runtime; `lamp` is drawn unlit; `dark` (tyres, trim, seats),
    `glass`, `steel` (rims, bars), `red` and `white` keep their colours.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import common
from common import MATERIALS, arg

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models")))
ONLY = arg("--only")
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with JEEP_* and SPORTS_* in crates/shared/src/vehicles/.
# `ride` is the height of the chassis centre above flat ground once the suspension has settled
# (JEEP_RIDE_HEIGHT, SPORTS_RIDE_HEIGHT): the wheel centres sit one wheel radius above the ground.
# The ground is `ride` below the origin, so the jeep's belly is 0.44 m off it and the sports car's 0.10 m.
# --------------------------------------------------------------------------------------------------
SPECS = {
    "jeep": dict(half=(1.0, 0.42, 1.95), wheel_radius=0.55, wheel_width=0.3, track=0.9, axle=1.25, ride=0.857),
    "sports": dict(half=(0.95, 0.3, 2.2), wheel_radius=0.32, wheel_width=0.26, track=0.82, axle=1.45, ride=0.397),
    # ROADSTER_*: the sports car's chassis and wheels under a different body.
    "roadster": dict(half=(0.95, 0.3, 2.2), wheel_radius=0.32, wheel_width=0.28, track=0.82, axle=1.45, ride=0.397),
}

MATERIAL_COLORS = {
    "paint": (0.8, 0.45, 0.1),
    "dark": (0.09, 0.09, 0.1),
    "glass": (0.55, 0.78, 0.9),
    "steel": (0.6, 0.62, 0.64),
    "lamp": (1.0, 0.96, 0.66),
    "red": (0.75, 0.05, 0.05),
    "white": (0.92, 0.92, 0.9),
}
NAMES = list(MATERIAL_COLORS)
PAINT, DARK, GLASS, STEEL, LAMP, RED, WHITE = range(len(NAMES))

scene = None
MATERIALS[:] = []


def reset_scene():
    global scene
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    MATERIALS[:] = []
    for name, rgb in MATERIAL_COLORS.items():
        m = bpy.data.materials.new(name)
        m.use_nodes = True
        bsdf = m.node_tree.nodes["Principled BSDF"]
        bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
        bsdf.inputs["Roughness"].default_value = 1.0
        m.diffuse_color = (*rgb, 1.0)
        MATERIALS.append(m)


class Part(common.Part):

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None):
        turn = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"), "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        m = Matrix.Translation(centre) @ turn
        verts = bmesh.ops.create_cone(
            self.bm,
            cap_ends=True,
            cap_tris=False,
            segments=segments,
            radius1=radius,
            radius2=radius if radius2 is None else radius2,
            depth=depth,
            matrix=m,
        )["verts"]
        self._tag(verts, mat)


def empty(name, location, parent=None):
    obj = bpy.data.objects.new(name, None)
    obj.location = location
    scene.collection.objects.link(obj)
    if parent:
        obj.parent = parent
    return obj


def mirrored(body, x, y, z, size, mat, rx=0.0):
    """The same box on both sides of the car, at +/-x."""
    for side in (-1, 1):
        body.box((side * x, y, z), size, mat, rx=rx)


# --------------------------------------------------------------------------------------------------
# Wheels and seats, shared by both cars
# --------------------------------------------------------------------------------------------------
def add_wheels(spec, lugs, rim_ratio):
    """Four wheels on steering pivots. Returns the objects built."""
    radius, width = spec["wheel_radius"], spec["wheel_width"]
    # The settled wheel centre; the suspension is the only thing that moves it from here.
    z = radius - spec["ride"]
    built = []
    for fore, y in (("F", spec["axle"]), ("R", -spec["axle"])):
        for side, x in (("L", -spec["track"]), ("R", spec["track"])):
            tag = fore + side
            pivot = empty(f"pivot_{tag}", (x, y, z))
            wheel = Part(f"wheel_{tag}", parent=pivot)
            wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=14 if lugs else 12)
            wheel.cylinder((0, 0, 0), radius * rim_ratio, width + 0.04, STEEL, axis="X", segments=10)
            # A bar across the rim, so that rolling can be seen.
            wheel.box((0, 0, 0), (width + 0.07, radius * 1.15, 0.07), DARK)
            for k in range(lugs):
                a = 2 * math.pi * k / lugs
                # Chunky tread blocks: tangent to the tyre, standing a little proud of it.
                wheel.box((0, (radius + 0.02) * math.cos(a), (radius + 0.02) * math.sin(a)), (width * 0.9, 0.17, 0.1), DARK, rx=a + math.pi / 2)
            built.append(wheel.build())
    return built


def add_seats(positions):
    for index, position in enumerate(positions):
        empty(f"seat_{index}", position)


# --------------------------------------------------------------------------------------------------
# The jeep: a tall-tyred open tub on a ladder frame, with a roll cage and a spare on the back
# --------------------------------------------------------------------------------------------------
def build_jeep():
    spec = SPECS["jeep"]
    body = Part("body")
    # Frame rails and cross members under the tub: the high belly is the point.
    mirrored(body, 0.5, 0.0, -0.33, (0.12, 3.5, 0.14), DARK)
    for y in (-1.25, 0.0, 1.25):
        body.box((0.0, y, -0.33), (1.1, 0.14, 0.12), DARK)
    # Tub: floor, sides, tailgate and dash, open on top.
    body.box((0.0, -0.4, -0.18), (1.6, 2.7, 0.1), PAINT)
    mirrored(body, 0.78, -0.4, 0.04, (0.1, 2.7, 0.4), PAINT)
    body.box((0.0, -1.72, 0.05), (1.66, 0.1, 0.5), PAINT)
    body.box((0.0, 0.88, 0.14), (1.66, 0.14, 0.6), PAINT)
    # Bonnet, grille and bumpers.
    body.box((0.0, 1.42, 0.12), (1.5, 1.1, 0.3), PAINT)
    body.box((0.0, 1.98, 0.1), (1.2, 0.06, 0.28), DARK)
    body.box((0.0, 2.02, -0.14), (1.96, 0.2, 0.14), DARK)
    body.box((0.0, -2.0, -0.14), (1.96, 0.2, 0.14), DARK)
    for side in (-1, 1):
        body.cylinder((side * 0.5, 2.03, 0.24), 0.12, 0.08, LAMP, axis="Y", segments=8)
        body.box((side * 0.62, -1.78, 0.18), (0.14, 0.05, 0.1), RED)
    # Flat fenders over all four wheels.
    for y in (1.25, -1.25):
        mirrored(body, 0.88, y, 0.34, (0.42, 0.9, 0.06), PAINT)
    # Seats: cushion and back for the front row and the bench behind.
    for y in (0.3, -0.75):
        for side in (-1, 1):
            body.box((side * 0.4, y, -0.04), (0.62, 0.56, 0.16), DARK)
            body.box((side * 0.4, y - 0.3, 0.26), (0.62, 0.12, 0.5), DARK, rx=0.12)
    # Windscreen, its posts, and a roll cage: two hoops tied by roof rails.
    body.box((0.0, 0.88, 0.62), (1.4, 0.04, 0.5), GLASS, rx=0.2)
    mirrored(body, 0.75, 0.84, 0.64, (0.07, 0.07, 0.8), STEEL, rx=0.2)
    body.box((0.0, 0.78, 1.04), (1.5, 0.07, 0.07), STEEL)
    mirrored(body, 0.75, -1.2, 0.62, (0.08, 0.08, 1.05), STEEL)
    body.box((0.0, -1.2, 1.12), (1.58, 0.08, 0.08), STEEL)
    mirrored(body, 0.75, -0.2, 1.08, (0.07, 2.1, 0.07), STEEL)
    # Spare wheel on the tailgate.
    body.cylinder((0.0, -2.14, 0.3), 0.5, 0.22, DARK, axis="Y", segments=12)
    body.cylinder((0.0, -2.26, 0.3), 0.28, 0.04, STEEL, axis="Y", segments=8)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=12, rim_ratio=0.6)
    add_seats([(-0.4, 0.3, 0.62), (0.4, 0.3, 0.62), (-0.4, -0.75, 0.62), (0.4, -0.75, 0.62)])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The sports car: a flat wedge, a roadster with a cut-down windscreen and a wing on the back
# --------------------------------------------------------------------------------------------------
def build_sports():
    spec = SPECS["sports"]
    body = Part("body")
    body.box((0.0, 0.0, -0.17), (1.5, 4.3, 0.1), DARK)
    # Nose and tail as side profiles, (y, z); the cockpit sits between them with sills at its sides.
    body.prism([(0.7, -0.2), (2.15, -0.2), (2.22, -0.02), (1.8, 0.12), (0.9, 0.2), (0.7, 0.2)], -0.78, 0.78, PAINT)
    body.prism([(-2.2, -0.2), (-2.2, 0.12), (-1.8, 0.22), (-0.9, 0.26), (-0.9, -0.2)], -0.78, 0.78, PAINT)
    mirrored(body, 0.74, -0.1, 0.0, (0.14, 1.7, 0.3), PAINT)
    # Cowl and a windscreen leaned well back.
    body.box((0.0, 0.62, 0.22), (1.5, 0.25, 0.12), PAINT)
    body.box((0.0, 0.46, 0.38), (1.4, 0.04, 0.3), GLASS, rx=0.55)
    # Two bucket seats.
    for side in (-1, 1):
        body.box((side * 0.35, -0.05, -0.08), (0.5, 0.55, 0.12), DARK)
        body.box((side * 0.35, -0.38, 0.14), (0.5, 0.1, 0.36), DARK, rx=0.15)
    # Stripe down the bonnet, fender flares, splitter, wing, lamps, exhausts.
    body.box((0.0, 1.4, 0.172), (0.3, 1.1, 0.02), WHITE, rx=-0.09)
    for y in (1.45, -1.45):
        mirrored(body, 0.84, y, 0.16, (0.24, 0.85, 0.12), PAINT)
    body.box((0.0, 2.2, -0.18), (1.7, 0.3, 0.04), DARK)
    mirrored(body, 0.5, -2.0, 0.3, (0.06, 0.06, 0.2), DARK)
    body.box((0.0, -2.05, 0.42), (1.7, 0.4, 0.05), PAINT)
    for side in (-1, 1):
        body.box((side * 0.5, 2.03, 0.04), (0.3, 0.1, 0.07), LAMP)
        body.box((side * 0.55, -2.22, 0.1), (0.3, 0.04, 0.07), RED)
        body.cylinder((side * 0.3, -2.3, -0.12), 0.06, 0.2, STEEL, axis="Y", segments=6)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.66)
    add_seats([(-0.35, -0.1, 0.4), (0.35, -0.1, 0.4)])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The Roadster (2026): a smooth, low two-seater wedge with a glass canopy under a black targa roof, a
# full-width light bar at the back and ten cold-gas nozzles: four in the rear diffuser that push it
# forward, and six along the underside (nose corners, sills, tail corners) that lift it.
# --------------------------------------------------------------------------------------------------
REAR_NOZZLES = [(-0.5, -2.24, -0.1), (-0.18, -2.24, -0.1), (0.18, -2.24, -0.1), (0.5, -2.24, -0.1)]
LIFT_NOZZLES = [(-0.62, 1.95, -0.21), (0.62, 1.95, -0.21), (-0.86, 0.0, -0.21), (0.86, 0.0, -0.21), (-0.62, -1.95, -0.21), (0.62, -1.95, -0.21)]


def build_roadster():
    spec = SPECS["roadster"]
    body = Part("body")
    body.box((0.0, 0.0, -0.17), (1.56, 4.3, 0.1), DARK)
    # One flowing side profile, (y, z), nose to tail: a long low bonnet rising to the cowl, then
    # the rear deck falling away to a short cut-off tail.
    body.prism(
        [(2.22, -0.18), (2.24, -0.04), (1.95, 0.06), (1.3, 0.14), (0.75, 0.22), (-0.6, 0.27), (-1.6, 0.26), (-2.12, 0.16), (-2.2, 0.0), (-2.2, -0.18)],
        -0.86, 0.86, PAINT,
    )
    # Wide haunches over the rear wheels and softer swells over the front ones.
    for y, size in ((-1.45, (0.16, 1.15, 0.2)), (1.45, (0.12, 0.95, 0.14))):
        mirrored(body, 0.86, y, 0.06, size, PAINT)
    # The canopy: a glass bubble over both seats, with the black targa panel on top of it.
    body.prism([(0.75, 0.22), (0.2, 0.52), (-0.45, 0.56), (-1.15, 0.32), (-1.15, 0.26), (0.75, 0.2)], -0.64, 0.64, GLASS)
    body.box((0.0, -0.15, 0.565), (1.18, 0.62, 0.035), DARK)
    # Slim headlamps and a black intake under the nose; a full-width light bar and a diffuser at the back.
    for side in (-1, 1):
        body.box((side * 0.55, 1.98, 0.05), (0.42, 0.08, 0.04), LAMP, rx=-0.2)
        # The side intakes that feed the air tanks.
        body.box((side * 0.86, -0.75, 0.05), (0.04, 0.5, 0.12), DARK)
    body.box((0.0, 2.22, -0.11), (1.1, 0.06, 0.08), DARK)
    body.box((0.0, -2.2, 0.11), (1.62, 0.04, 0.05), RED)
    body.box((0.0, -2.16, -0.1), (1.5, 0.14, 0.14), DARK)
    # The nozzles: steel mouths, the rear ones facing back, the lift ones facing down.
    for x, y, z in REAR_NOZZLES:
        body.cylinder((x, y, z), 0.06, 0.12, STEEL, axis="Y", segments=8, radius2=0.045)
    for x, y, z in LIFT_NOZZLES:
        body.cylinder((x, y, z), 0.07, 0.08, STEEL, axis="Z", segments=8)
    # Two low bucket seats in the cockpit.
    for side in (-1, 1):
        body.box((side * 0.33, -0.15, 0.22), (0.44, 0.5, 0.06), DARK)
        body.box((side * 0.33, -0.45, 0.36), (0.44, 0.08, 0.3), DARK, rx=0.25)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.72)
    add_seats([(-0.33, -0.12, 0.42), (0.33, -0.12, 0.42)])
    for index, (x, y, z) in enumerate(REAR_NOZZLES):
        empty(f"nozzle_rear_{index}", (x, y - 0.07, z))
    for index, (x, y, z) in enumerate(LIFT_NOZZLES):
        empty(f"nozzle_lift_{index}", (x, y, z - 0.05))
    return [body_obj, *wheels]


BUILDERS = {"jeep": build_jeep, "sports": build_sports, "roadster": build_roadster}


# --------------------------------------------------------------------------------------------------
# Export and previews
# --------------------------------------------------------------------------------------------------
def export(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    kwargs = dict(
        filepath=path,
        export_format="GLB",
        use_selection=True,
        export_yup=True,
        export_apply=True,
        export_animations=False,
        export_skins=False,
        export_cameras=False,
        export_lights=False,
        export_extras=False,
        export_materials="EXPORT",
    )
    valid = bpy.ops.export_scene.gltf.get_rna_type().properties.keys()
    bpy.ops.export_scene.gltf(**{k: v for k, v in kwargs.items() if k in valid})


def preview(directory, name):
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
        "three": ("PERSP", Vector((6.5, 6.5, 3.2)), Vector((0, 0, 0.0)), 0),
        "side": ("ORTHO", Vector((12, 0, 0.0)), Vector((0, 0, 0.0)), 6.5),
        "front": ("ORTHO", Vector((0, 12, 0.0)), Vector((0, 0, 0.0)), 3.6),
        "rear": ("PERSP", Vector((-5.5, -6.0, 3.0)), Vector((0, 0, 0.0)), 0),
    }
    for view, (kind, pos, target, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = pos
        cam.rotation_euler = (target - pos).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}_{view}.png")
        bpy.ops.render.render(write_still=True)


for name in [ONLY] if ONLY else BUILDERS:
    reset_scene()
    objects = BUILDERS[name]()
    export(os.path.join(OUT, f"{name}.glb"))
    print(name, "tris:", sum(len(p.vertices) - 2 for o in objects for p in o.data.polygons))
    if PREVIEW:
        preview(PREVIEW, name)
