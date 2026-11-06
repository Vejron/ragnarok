"""
Builds the three cars, an open jeep, a low sports car and the thruster Roadster, and exports them
to client/public/models/.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- \
        [--out DIR] [--only jeep|sports|roadster|snowmobile|golf_cart|technical] [--preview DIR]

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
    # SNOWMOBILE_*: `track` is half the ski stance, `axle` where the skis and the rear idlers are.
    "snowmobile": dict(half=(0.55, 0.3, 1.6), wheel_radius=0.22, wheel_width=0.16, track=0.55, axle=1.2, ride=0.559),
    # GOLF_CART_*: small wheels close in under a short body.
    "golf_cart": dict(half=(0.6, 0.28, 1.2), wheel_radius=0.23, wheel_width=0.18, track=0.5, axle=0.82, ride=0.397),
    # TECHNICAL_*: big tyres on a tall truck, its belly high off the ground.
    "technical": dict(half=(0.95, 0.4, 2.45), wheel_radius=0.58, wheel_width=0.36, track=0.85, axle=1.6, ride=1.08),
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


# --------------------------------------------------------------------------------------------------
# The snowmobile: a modern mountain sled. A sharp, narrow, rider-forward body over a long deep-lugged
# track, two skis on A-arms, tall risers and a long tandem seat. The skis hang from `pivot_FL/FR` and
# steer but do not roll (there are no `wheel_F*`); the rear idlers inside the track are `wheel_RL/RR`.
# --------------------------------------------------------------------------------------------------
def build_snowmobile():
    spec = SPECS["snowmobile"]
    ground = -spec["ride"]
    body = Part("body")
    # The track: a side profile (y, z) of the belt in its tunnel, from the tail up over the rear idlers,
    # flat along the ground, then rising to the drive sprocket under the footwells.
    track = [(-1.68, ground + 0.16), (-1.58, ground + 0.02), (0.15, ground + 0.02), (0.55, ground + 0.24), (0.45, -0.12), (-1.62, -0.12)]
    body.prism(track, -0.19, 0.19, DARK)
    # Deep paddles under the belt and over its return run.
    y = -1.55
    while y < 0.12:
        body.box((0.0, y, ground + 0.01), (0.4, 0.035, 0.06), DARK)
        y += 0.13
    for k in range(8):
        body.box((0.0, -1.5 + k * 0.2, -0.105), (0.4, 0.04, 0.04), DARK)
    # The tunnel over the track, its cooler fins, the snowflap and the tail light at its end.
    body.box((0.0, -0.75, -0.06), (0.48, 1.9, 0.08), PAINT)
    mirrored(body, 0.23, -0.75, -0.14, (0.03, 1.9, 0.18), PAINT)
    body.box((0.0, -1.62, -0.36), (0.46, 0.02, 0.36), DARK)
    body.box((0.0, -1.68, -0.08), (0.24, 0.05, 0.05), RED)
    # Running boards: open steel footwells either side of the tunnel.
    mirrored(body, 0.37, -0.15, -0.12, (0.24, 1.1, 0.025), STEEL)
    mirrored(body, 0.49, -0.15, -0.1, (0.025, 1.1, 0.05), STEEL)
    # The belly pan and the engine bay's side panels, under a sharp angular hood.
    body.prism([(0.25, -0.3), (1.55, -0.3), (1.85, -0.18), (1.85, -0.06), (0.25, -0.06)], -0.4, 0.4, DARK)
    body.prism([(0.25, -0.06), (1.85, -0.06), (1.7, 0.08), (1.1, 0.26), (0.7, 0.34), (0.25, 0.34)], -0.42, 0.42, PAINT)
    # The nose: a narrower wedge proud of the hood, like a modern sled's pointed front.
    body.prism([(1.2, 0.0), (2.0, -0.14), (2.02, -0.04), (1.45, 0.24), (1.2, 0.28)], -0.24, 0.24, PAINT)
    # White graphics down both sides of the hood, and vents.
    mirrored(body, 0.425, 1.05, 0.08, (0.01, 0.9, 0.06), WHITE, rx=0.22)
    for y in (0.55, 0.7, 0.85):
        mirrored(body, 0.425, y, -0.02, (0.012, 0.06, 0.18), DARK)
    # LED headlights: two slim strips in the nose and a centre lamp.
    for side in (-1, 1):
        body.box((side * 0.14, 1.84, 0.0), (0.16, 0.05, 0.035), LAMP, rx=-0.5)
    body.box((0.0, 1.93, -0.08), (0.12, 0.04, 0.03), LAMP)
    # A tall, narrow windscreen leaned back over the gauge.
    body.box((0.0, 0.76, 0.56), (0.46, 0.025, 0.46), GLASS, rx=0.4)
    body.box((0.0, 0.62, 0.38), (0.16, 0.12, 0.06), DARK)
    # Front bumper loop, and the A-arms and shocks that carry the skis.
    body.cylinder((0.0, 1.98, -0.2), 0.022, 0.7, STEEL, axis="X", segments=6)
    for side in (-1, 1):
        for y in (spec["axle"] - 0.12, spec["axle"] + 0.12):
            body.box((side * 0.32, y, -0.3), (0.36, 0.04, 0.035), STEEL)
        body.cylinder((side * 0.36, spec["axle"], -0.12), 0.035, 0.36, RED, axis="Z", segments=8)
    # Tall risers to wide handlebars with hand guards, and the grips.
    body.cylinder((0.0, 0.5, 0.55), 0.03, 0.42, STEEL, axis="Z", segments=8)
    body.cylinder((0.0, 0.48, 0.77), 0.018, 0.82, STEEL, axis="X", segments=6)
    for side in (-1, 1):
        body.cylinder((side * 0.36, 0.48, 0.77), 0.026, 0.12, DARK, axis="X", segments=6)
        body.box((side * 0.4, 0.56, 0.8), (0.03, 0.18, 0.12), PAINT)
    # The long, thin tandem seat, rising behind for the passenger, with a grab handle and a rack.
    body.box((0.0, -0.25, 0.4), (0.32, 1.1, 0.14), DARK)
    body.box((0.0, -0.95, 0.48), (0.32, 0.5, 0.18), DARK)
    # The seat's base on the tunnel: slim side panels, tapering in at the tail.
    body.prism([(0.3, -0.02), (0.3, 0.34), (-1.2, 0.34), (-1.45, 0.2), (-1.45, -0.02)], -0.2, 0.2, PAINT)
    body.prism([(0.3, -0.02), (0.3, 0.2), (-0.6, 0.2), (-0.6, -0.02)], -0.22, 0.22, WHITE)
    body.box((0.0, -1.24, 0.6), (0.3, 0.04, 0.12), STEEL)
    mirrored(body, 0.17, -1.42, 0.38, (0.025, 0.4, 0.025), STEEL)
    body.box((0.0, -1.62, 0.38), (0.36, 0.025, 0.025), STEEL)
    body_obj = body.build()

    built = [body_obj]
    # The skis: long and narrow with an upturned tip, a keel and a spindle up to the A-arms.
    z = spec["wheel_radius"] - spec["ride"]
    for side, x in (("L", -spec["track"]), ("R", spec["track"])):
        pivot = empty(f"pivot_F{side}", (x, spec["axle"], z))
        ski = Part(f"ski_F{side}", parent=pivot)
        floor = -spec["wheel_radius"]
        ski.box((0.0, -0.05, floor + 0.025), (0.16, 1.15, 0.04), DARK)
        ski.box((0.0, 0.6, floor + 0.09), (0.15, 0.22, 0.035), DARK, rx=0.75)
        ski.box((0.0, -0.05, floor + 0.005), (0.03, 1.0, 0.02), STEEL)
        ski.box((0.0, 0.0, floor + 0.08), (0.06, 0.2, 0.09), PAINT)
        ski.cylinder((0.0, 0.0, floor + 0.2), 0.03, 0.26, STEEL, axis="Z", segments=6)
        built.append(ski.build())
    # The rear idlers inside the track, which roll.
    for side, x in (("L", -0.22), ("R", 0.22)):
        pivot = empty(f"pivot_R{side}", (x, -spec["axle"], z))
        wheel = Part(f"wheel_R{side}", parent=pivot)
        wheel.cylinder((0, 0, 0), 0.19, 0.05, STEEL, axis="X", segments=12)
        wheel.box((0, 0, 0), (0.06, 0.3, 0.05), DARK)
        built.append(wheel.build())
    add_seats([(0.0, -0.1, 1.05), (0.0, -0.8, 1.15)])
    return built


# --------------------------------------------------------------------------------------------------
# The golf cart: a short two-seater on small wheels, a rounded cowl at the front, a bench seat over the
# batteries, a white canopy roof on four struts, and two golf bags standing on a rack at the back.
# --------------------------------------------------------------------------------------------------
def build_golf_cart():
    spec = SPECS["golf_cart"]
    body = Part("body")
    # The frame and the floor of the footwell, between the cowl and the seat.
    body.box((0.0, 0.0, -0.22), (1.0, 2.3, 0.08), DARK)
    body.box((0.0, 0.25, -0.16), (1.1, 0.7, 0.04), DARK)
    # The front cowl as a side profile, (y, z): a rounded nose over the front wheels, up to the dash.
    body.prism([(0.58, -0.22), (1.18, -0.22), (1.24, -0.05), (1.16, 0.1), (0.85, 0.2), (0.58, 0.32), (0.5, 0.32), (0.5, -0.22)], -0.6, 0.6, PAINT)
    # The seat's base over the batteries, a white bench cushion and a back leaned a little.
    body.box((0.0, -0.3, -0.02), (1.2, 0.7, 0.4), PAINT)
    body.box((0.0, -0.28, 0.22), (1.06, 0.6, 0.1), WHITE)
    body.box((0.0, -0.6, 0.5), (1.06, 0.1, 0.46), WHITE, rx=0.15)
    mirrored(body, 0.58, -0.32, 0.36, (0.05, 0.5, 0.05), STEEL)
    # The tail over the rear wheels and the motor: a low deck, falling to a cut-off back.
    body.prism([(-0.65, -0.22), (-0.65, 0.18), (-1.12, 0.14), (-1.22, 0.02), (-1.22, -0.22)], -0.6, 0.6, PAINT)
    # Bumpers, headlamps and tail lamps.
    body.box((0.0, 1.26, -0.17), (1.0, 0.08, 0.1), DARK)
    body.box((0.0, -1.25, -0.17), (1.16, 0.08, 0.1), DARK)
    for side in (-1, 1):
        body.cylinder((side * 0.36, 1.2, 0.04), 0.07, 0.06, LAMP, axis="Y", segments=8)
        body.box((side * 0.48, -1.23, 0.06), (0.12, 0.03, 0.06), RED)
    # The steering column and wheel before the driver (on the left), and a windscreen on the dash.
    body.box((-0.28, 0.4, 0.38), (0.05, 0.05, 0.36), DARK, rx=0.5)
    body.cylinder((-0.28, 0.3, 0.55), 0.17, 0.03, DARK, axis="Y", segments=12)
    body.box((0.0, 0.52, 0.85), (1.04, 0.025, 1.0), GLASS, rx=0.12)
    # Four struts up to the canopy roof.
    mirrored(body, 0.55, 0.55, 0.9, (0.04, 0.04, 1.15), STEEL, rx=0.12)
    mirrored(body, 0.55, -0.72, 0.85, (0.04, 0.04, 1.2), STEEL)
    body.box((0.0, -0.1, 1.48), (1.24, 1.75, 0.06), WHITE)
    body.box((0.0, -0.1, 1.43), (1.2, 1.7, 0.04), DARK)
    # The bag rack and two golf bags standing on it, club heads showing at the top.
    body.box((0.0, -1.05, 0.2), (0.9, 0.36, 0.04), STEEL)
    body.box((0.0, -0.9, 0.5), (0.9, 0.04, 0.04), STEEL)
    for side, mat in ((-1, RED), (1, DARK)):
        x = side * 0.22
        body.cylinder((x, -1.05, 0.55), 0.13, 0.7, mat, axis="Z", segments=10)
        body.cylinder((x, -1.05, 0.91), 0.14, 0.04, WHITE, axis="Z", segments=10)
        for dx, dy, h in ((-0.05, 0.04, 0.12), (0.05, 0.03, 0.17), (0.0, -0.05, 0.08)):
            body.box((x + dx, -1.05 + dy, 0.93 + h / 2), (0.02, 0.02, h), STEEL)
            body.box((x + dx, -1.05 + dy + 0.03, 0.93 + h), (0.04, 0.08, 0.04), STEEL)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.6)
    # GOLF_CART_SEAT_HEADS, in this frame: x, -z, y.
    add_seats([(-0.28, -0.25, 0.82), (0.28, -0.25, 0.82)])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The technical: a long-bonnet pickup on big tyres, its cab roofed but open at the sides, a snorkel up
# the windscreen post, a bull bar, and a heavy machine gun on a post in the open bed. The gun's mount
# (`turret`) yaws on the post and the gun (`gun`, its child) pitches, both about TECHNICAL_MG_PIVOT.
# --------------------------------------------------------------------------------------------------
TECHNICAL_MG_PIVOT = (0.0, -0.9, 1.45)


def build_technical():
    spec = SPECS["technical"]
    body = Part("body")
    # The ladder frame, high off the ground.
    mirrored(body, 0.45, 0.0, -0.32, (0.12, 4.7, 0.16), DARK)
    for y in (2.0, 0.6, -0.9, -2.2):
        body.box((0.0, y, -0.32), (0.95, 0.12, 0.12), DARK)
    # The long bonnet, its grille, round headlamps, and a bull bar over the bumper.
    body.box((0.0, 1.68, 0.1), (1.7, 1.35, 0.7), PAINT)
    body.box((0.0, 2.37, 0.1), (1.36, 0.06, 0.46), DARK)
    for x in (-0.24, 0.0, 0.24):
        body.box((x, 2.41, 0.1), (0.04, 0.03, 0.4), STEEL)
    for side in (-1, 1):
        body.cylinder((side * 0.6, 2.4, 0.2), 0.11, 0.06, LAMP, axis="Y", segments=10)
    body.box((0.0, 2.52, -0.2), (1.96, 0.18, 0.22), DARK)
    mirrored(body, 0.4, 2.56, 0.1, (0.06, 0.06, 0.5), STEEL)
    body.box((0.0, 2.56, 0.34), (0.86, 0.06, 0.06), STEEL)
    # Flared arches over all four tyres.
    for y in (spec["axle"], -spec["axle"]):
        mirrored(body, 0.93, y, 0.12, (0.28, 1.3, 0.08), PAINT)
        mirrored(body, 0.99, y, 0.0, (0.06, 1.3, 0.2), DARK)
    # The cab: its lower body, the bench and the steering wheel; the roof on four posts, glass in front
    # and behind, and nothing in the doors, so whoever sits there can be seen and shot.
    body.box((0.0, 0.42, 0.1), (1.8, 1.15, 0.7), PAINT)
    for side in (-1, 1):
        body.box((side * 0.42, 0.25, 0.5), (0.62, 0.55, 0.14), DARK)
        body.box((side * 0.42, -0.05, 0.85), (0.62, 0.1, 0.62), DARK, rx=-0.1)
    body.box((-0.42, 0.68, 0.72), (0.05, 0.05, 0.4), DARK, rx=0.6)
    body.cylinder((-0.42, 0.6, 0.85), 0.18, 0.03, DARK, axis="Y", segments=12)
    body.box((0.0, 0.42, 1.25), (1.78, 1.2, 0.07), PAINT)
    mirrored(body, 0.86, 0.94, 0.88, (0.06, 0.06, 0.76), PAINT, rx=0.25)
    mirrored(body, 0.86, -0.13, 0.85, (0.08, 0.08, 0.8), PAINT)
    body.box((0.0, 0.94, 0.86), (1.64, 0.03, 0.7), GLASS, rx=0.25)
    body.box((0.0, -0.13, 0.82), (1.6, 0.04, 0.5), GLASS)
    # A snorkel up the right windscreen post.
    body.cylinder((0.98, 1.05, 0.6), 0.06, 1.2, DARK, segments=8)
    body.box((0.98, 1.1, 1.24), (0.13, 0.22, 0.12), DARK)
    # The open bed: floor, sides and tailgate, and the bumper and tail lamps under it.
    body.box((0.0, -1.33, 0.11), (1.8, 2.3, 0.08), PAINT)
    mirrored(body, 0.88, -1.33, 0.1, (0.06, 2.3, 0.7), PAINT)
    body.box((0.0, -2.45, 0.1), (1.82, 0.06, 0.7), PAINT)
    mirrored(body, 0.88, -1.33, 0.47, (0.09, 2.3, 0.05), STEEL)
    body.box((0.0, -2.52, -0.2), (1.8, 0.14, 0.18), DARK)
    for side in (-1, 1):
        body.box((side * 0.8, -2.49, 0.3), (0.14, 0.03, 0.12), RED)
    # The gun's post, braced to the bed's floor.
    px, py, pz = TECHNICAL_MG_PIVOT
    body.cylinder((px, py, (0.15 + pz - 0.1) / 2), 0.07, pz - 0.25, STEEL, segments=8)
    body.box((px, py, 0.17), (0.5, 0.5, 0.04), STEEL)
    for side in (-1, 1):
        body.box((side * 0.16, py, 0.45), (0.04, 0.04, 0.65), STEEL, rx=0.0)
    body_obj = body.build()
    # The mount yaws on the post: its cradle, and a shield before the gunner.
    turret = Part("turret", location=TECHNICAL_MG_PIVOT, parent=body_obj)
    turret.cylinder((0, 0, -0.08), 0.1, 0.08, DARK, segments=10)
    mirrored(turret, 0.11, 0.0, 0.0, (0.04, 0.14, 0.2), DARK)
    turret.box((0.0, 0.32, 0.1), (0.62, 0.03, 0.42), DARK)
    turret_obj = turret.build()
    # The gun pitches in the cradle: receiver, a long barrel with a muzzle brake, spade grips and the box.
    gun = Part("gun", parent=turret_obj)
    gun.box((0.0, -0.12, 0.02), (0.13, 0.55, 0.15), DARK)
    gun.cylinder((0.0, 0.62, 0.03), 0.032, 1.1, DARK, axis="Y", segments=8)
    gun.cylinder((0.0, 1.15, 0.03), 0.05, 0.1, DARK, axis="Y", segments=8)
    gun.box((0.0, -0.44, 0.0), (0.18, 0.04, 0.12), STEEL)
    gun.box((0.14, -0.08, -0.03), (0.13, 0.22, 0.16), STEEL)
    gun_obj = gun.build()
    wheels = add_wheels(spec, lugs=14, rim_ratio=0.5)
    # TECHNICAL_SEAT_HEADS, in this frame: x, -z, y.
    add_seats([(-0.42, 0.35, 1.0), (0.42, 0.35, 1.0), (0.0, -1.65, 1.79)])
    return [body_obj, turret_obj, gun_obj, *wheels]


BUILDERS = {"jeep": build_jeep, "sports": build_sports, "roadster": build_roadster, "snowmobile": build_snowmobile, "golf_cart": build_golf_cart, "technical": build_technical}


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
