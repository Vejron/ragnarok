"""
Builds the three cars, an open jeep, a low sports car and the thruster Roadster, and exports them
to client/public/models/.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- \
        [--out DIR] [--only jeep|sports|roadster|snowmobile|golf_cart|technical|snowracer|bus|amv] [--preview DIR]

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
    # SNOWRACER_*: `track` is half the runners' stance, `axle` where the steering ski is.
    "snowracer": dict(half=(0.3, 0.15, 0.75), wheel_radius=0.1, wheel_width=0.1, track=0.25, axle=0.55, ride=0.202),
    # BUS_*: `half` is the frame under the floor; the body stands on it up to 2.3 m over the origin.
    "bus": dict(half=(1.25, 0.5, 6.0), wheel_radius=0.5, wheel_width=0.3, track=1.05, axle=3.0, ride=1.057),
    # AMV_*: eight wheels on four axles (`axles`, front first in the game's z), under an armoured hull.
    "amv": dict(half=(1.4, 0.925, 3.85), wheel_radius=0.6, wheel_width=0.4, track=1.12, axle=2.35, axles=(-2.35, -0.85, 0.85, 2.35), ride=1.35),
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


# --------------------------------------------------------------------------------------------------
# The snowracer: a steerable sled. A frame of bent steel tube in the car's paint, two long black
# runners whose tails curl up behind, a padded black seat for two along the top of the frame, a
# sprung steering column up to BMX-style bars, one short ski under it, toothed brake claws either
# side, and a small motor slung under the seat. The ski hangs from `pivot_FL`, in the middle, and
# steers but does not roll (there is no `wheel_*`, and no `pivot_FR`: the physics' front pair both
# stand for this one ski). The runners are part of the body.
# --------------------------------------------------------------------------------------------------
def tube(part, a, b, radius, mat, segments=6):
    """A round tube from point `a` to point `b`."""
    a, b = Vector(a), Vector(b)
    turn = Vector((0.0, 0.0, 1.0)).rotation_difference(b - a).to_matrix().to_4x4()
    m = Matrix.Translation((a + b) / 2) @ turn
    verts = bmesh.ops.create_cone(part.bm, cap_ends=True, cap_tris=False, segments=segments, radius1=radius, radius2=radius, depth=(b - a).length, matrix=m)["verts"]
    part._tag(verts, mat)


def build_snowracer():
    spec = SPECS["snowracer"]
    g = -spec["ride"]
    head_y = spec["axle"] - 0.02
    body = Part("body")
    for side in (-1, 1):
        x = side * 0.23
        # The runner: a long flat strip, its tip turned up a little in front and its tail curled up behind.
        body.box((x, -0.2, g + 0.008), (0.075, 1.1, 0.016), DARK)
        body.box((x, 0.38, g + 0.025), (0.07, 0.1, 0.014), DARK, rx=0.35)
        body.box((x, -0.8, g + 0.05), (0.07, 0.14, 0.014), DARK, rx=-0.9)
        # The frame: a low rail riveted along the runner, swept in to the steering head in front, and
        # a hoop rising from the runner's tail to carry the seat.
        tube(body, (x, -0.72, g + 0.035), (x, 0.2, g + 0.035), 0.016, PAINT)
        tube(body, (x, 0.2, g + 0.035), (side * 0.05, head_y - 0.03, g + 0.12), 0.016, PAINT)
        tube(body, (x, -0.72, g + 0.035), (side * 0.17, -0.8, g + 0.2), 0.016, PAINT)
        tube(body, (side * 0.17, -0.8, g + 0.2), (side * 0.17, 0.05, g + 0.2), 0.016, PAINT)
        tube(body, (side * 0.17, 0.05, g + 0.2), (side * 0.04, head_y - 0.03, g + 0.2), 0.016, PAINT)
        # Rivets of the rail to the runner.
        for y in (-0.55, -0.15, 0.15):
            body.box((x, y, g + 0.022), (0.03, 0.03, 0.02), STEEL)
        # The brake claw: a toothed steel plate on a lever beside the frame, held up off the snow.
        body.box((side * 0.33, 0.22, g + 0.06), (0.1, 0.13, 0.008), STEEL, rx=0.4)
        for k in range(3):
            body.box((side * (0.29 + 0.04 * k), 0.165, g + 0.025), (0.012, 0.012, 0.03), STEEL)
        tube(body, (side * 0.3, 0.27, g + 0.08), (side * 0.2, 0.12, g + 0.2), 0.01, STEEL)
    tube(body, (-0.17, -0.8, g + 0.2), (0.17, -0.8, g + 0.2), 0.016, PAINT)
    tube(body, (-0.23, 0.0, g + 0.035), (0.23, 0.0, g + 0.035), 0.012, PAINT)
    # The padded seat for two along the top of the frame, with a white flash on its top.
    body.box((0.0, -0.36, g + 0.27), (0.34, 0.92, 0.12), DARK)
    body.box((0.0, -0.4, g + 0.25), (0.36, 0.86, 0.08), DARK)
    body.box((0.0, -0.36, g + 0.331), (0.2, 0.42, 0.004), WHITE)
    # The steering head: a short tube in the paint with the column's spring below it, and a little lamp.
    tube(body, (0.0, head_y, g + 0.12), (0.0, head_y - 0.03, g + 0.4), 0.032, PAINT, segments=8)
    tube(body, (0.0, head_y + 0.005, g + 0.07), (0.0, head_y, g + 0.12), 0.022, STEEL, segments=8)
    body.box((0.0, head_y - 0.05, g + 0.4), (0.09, 0.08, 0.04), PAINT)
    body.box((0.0, head_y + 0.035, g + 0.31), (0.06, 0.02, 0.04), LAMP)
    # BMX-style bars: a stem, two uprights splayed out and back, a padded crossbar and the grips.
    tube(body, (0.0, head_y - 0.03, g + 0.4), (0.0, head_y - 0.06, g + 0.47), 0.018, DARK)
    for side in (-1, 1):
        tube(body, (side * 0.06, head_y - 0.06, g + 0.47), (side * 0.2, head_y - 0.12, g + 0.7), 0.013, DARK)
        tube(body, (side * 0.2, head_y - 0.12, g + 0.7), (side * 0.32, head_y - 0.14, g + 0.71), 0.022, DARK, segments=8)
    tube(body, (-0.06, head_y - 0.06, g + 0.47), (0.06, head_y - 0.06, g + 0.47), 0.013, DARK)
    body.box((0.0, head_y - 0.1, g + 0.6), (0.3, 0.06, 0.06), DARK)
    body.box((0.0, head_y - 0.069, g + 0.6), (0.18, 0.002, 0.03), WHITE)
    # The motor under the seat, between the runners, and its little exhaust out to the side.
    body.box((0.0, -0.4, g + 0.13), (0.2, 0.26, 0.1), DARK)
    body.box((0.0, -0.26, g + 0.13), (0.16, 0.03, 0.08), STEEL)
    tube(body, (0.1, -0.45, g + 0.12), (0.2, -0.5, g + 0.12), 0.015, STEEL)
    body_obj = body.build()

    # The steering ski: short and broad, an upturned tip, a binding block and the post up into the spring.
    z = spec["wheel_radius"] - spec["ride"]
    pivot = empty("pivot_FL", (0.0, spec["axle"], z))
    ski = Part("ski_F", parent=pivot)
    floor = -spec["wheel_radius"]
    ski.box((0.0, -0.05, floor + 0.01), (0.17, 0.46, 0.02), DARK)
    ski.box((0.0, 0.24, floor + 0.05), (0.16, 0.14, 0.018), DARK, rx=0.6)
    ski.box((0.0, -0.06, floor + 0.04), (0.08, 0.14, 0.04), DARK)
    ski.box((0.0, -0.02, floor + 0.08), (0.03, 0.03, 0.1), STEEL)
    add_seats([(0.0, 0.0, 0.85), (0.0, -0.5, 0.85)])
    return [body_obj, ski.build()]


# --------------------------------------------------------------------------------------------------
# The bus: a twelve-metre city bus. A tub on the frame with the floor a little over the origin, open
# window frames all round above the sill (so the riders show), a flat roof, a glass windscreen, two
# doors on the right, and a seat at each of BUS_SEAT_HEADS.
# --------------------------------------------------------------------------------------------------
# The rows of passenger seats along the bus (Blender y), front to back, and where the driver sits.
BUS_ROWS = (3.2, 1.7, 0.2, -1.3, -2.8, -4.3)
BUS_DRIVER = (-0.75, 4.9)
# Heights over the origin: the skirt's foot, the floor, the window sill, the roof's underside and its top.
BUS_FOOT, BUS_FLOOR, BUS_SILL, BUS_EAVE, BUS_TOP = -0.7, 0.1, 0.75, 1.95, 2.3


def build_bus():
    spec = SPECS["bus"]
    half_x, half_y = spec["half"][0], spec["half"][2]
    axle, arch = spec["axle"], spec["wheel_radius"] + 0.2
    body = Part("body")
    # The underbody, full width between the wheel arches and narrow at them, so the wheels show.
    under = (BUS_FLOOR + BUS_FOOT) / 2
    for y0, y1 in ((-half_y, -axle - arch), (-axle + arch, axle - arch), (axle + arch, half_y)):
        body.box((0.0, (y0 + y1) / 2, under), (2 * half_x, y1 - y0, BUS_FLOOR - BUS_FOOT), PAINT)
    for y in (-axle, axle):
        body.box((0.0, y, under), (2 * (spec["track"] - spec["wheel_width"]), 2 * arch, BUS_FLOOR - BUS_FOOT), DARK)
        # The top of the arch, over the wheel.
        body.box((0.0, y, BUS_FLOOR - 0.04), (2 * half_x, 2 * arch, 0.08), PAINT)
    # The sides up to the sill, with a white band under the windows, and the roof.
    wall = (BUS_SILL + BUS_FLOOR) / 2
    mirrored(body, half_x - 0.04, 0.0, wall, (0.08, 2 * half_y, BUS_SILL - BUS_FLOOR), PAINT)
    mirrored(body, half_x - 0.02, 0.0, BUS_SILL - 0.09, (0.06, 2 * half_y - 0.1, 0.14), WHITE)
    body.box((0.0, 0.0, (BUS_EAVE + BUS_TOP) / 2), (2 * half_x, 2 * half_y, BUS_TOP - BUS_EAVE), PAINT)
    body.box((0.0, -0.4, BUS_TOP + 0.06), (1.5, 2.6, 0.12), WHITE)
    # Window pillars between the rows, and one at each corner.
    pillar = (BUS_EAVE + BUS_SILL) / 2
    for y in (-half_y + 0.06, -5.05, -3.55, -2.05, -0.55, 0.95, 2.45, 3.95, half_y - 0.06):
        mirrored(body, half_x - 0.05, y, pillar, (0.1, 0.12, BUS_EAVE - BUS_SILL), PAINT)
    # The front: a panel under the windscreen, the glass, a destination board over it and the lamps.
    body.box((0.0, half_y - 0.05, (BUS_SILL + 0.1 + BUS_FOOT) / 2), (2 * half_x, 0.1, BUS_SILL + 0.1 - BUS_FOOT), PAINT)
    body.box((0.0, half_y - 0.04, (BUS_EAVE + BUS_SILL + 0.1) / 2), (2 * half_x - 0.2, 0.04, BUS_EAVE - BUS_SILL - 0.1), GLASS)
    body.box((0.0, half_y + 0.01, BUS_EAVE + 0.17), (1.7, 0.04, 0.24), DARK)
    body.box((0.0, half_y + 0.03, BUS_EAVE + 0.17), (1.1, 0.03, 0.12), LAMP)
    body.box((0.0, half_y + 0.04, BUS_FOOT + 0.1), (2 * half_x + 0.06, 0.14, 0.24), DARK)
    for side in (-1, 1):
        body.cylinder((side * 0.9, half_y + 0.02, -0.2), 0.13, 0.06, LAMP, axis="Y", segments=8)
        # Mirrors on arms off the front corners.
        body.box((side * (half_x + 0.22), half_y - 0.1, 1.55), (0.44, 0.05, 0.05), DARK)
        body.box((side * (half_x + 0.42), half_y - 0.14, 1.4), (0.08, 0.14, 0.4), DARK)
    # The back: a panel with the engine's grille, a window, tail lamps and the bumper.
    body.box((0.0, -half_y + 0.05, (1.15 + BUS_FOOT) / 2), (2 * half_x, 0.1, 1.15 - BUS_FOOT), PAINT)
    body.box((0.0, -half_y + 0.04, (BUS_EAVE + 1.15) / 2), (2 * half_x - 0.3, 0.04, BUS_EAVE - 1.15), GLASS)
    body.box((0.0, -half_y - 0.01, -0.15), (1.6, 0.04, 0.6), DARK)
    body.box((0.0, -half_y - 0.04, BUS_FOOT + 0.1), (2 * half_x + 0.06, 0.14, 0.24), DARK)
    for side in (-1, 1):
        body.box((side * 1.05, -half_y - 0.01, 0.55), (0.22, 0.04, 0.5), RED)
    # Two doors on the right: a dark frame down to the step, with two glass leaves in it.
    for y in (4.6, -0.55):
        body.box((half_x - 0.01, y, (BUS_EAVE + BUS_FOOT + 0.25) / 2), (0.06, 1.3, BUS_EAVE - BUS_FOOT - 0.25), DARK)
        for leaf in (-0.31, 0.31):
            body.box((half_x + 0.01, y + leaf, (BUS_EAVE + BUS_FOOT + 0.45) / 2), (0.05, 0.54, BUS_EAVE - BUS_FOOT - 0.55), GLASS)
    # The driver's seat, dash and wheel, and a bench seat with a back at each passenger's place.
    seats = [(BUS_DRIVER[0], BUS_DRIVER[1])] + [(side * 0.85, y) for y in BUS_ROWS for side in (-1, 1)]
    for x, y in seats:
        body.box((x, y - 0.05, BUS_FLOOR + 0.25), (0.6, 0.5, 0.5), DARK)
        body.box((x, y - 0.36, BUS_FLOOR + 0.72), (0.6, 0.1, 0.6), DARK)
    body.box((BUS_DRIVER[0], half_y - 0.45, BUS_SILL - 0.05), (0.9, 0.5, 0.3), DARK)
    body.cylinder((BUS_DRIVER[0], BUS_DRIVER[1] + 0.5, BUS_SILL + 0.22), 0.24, 0.04, DARK, axis="Y", segments=12)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.55)
    # BUS_SEAT_HEADS, in this frame: x, -z, y.
    add_seats([(x, y, 1.3) for x, y in seats])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The AMV XA-361: an eight-wheeled armoured carrier with a mortar turret. The upper hull overhangs the
# wheels, the lower hull runs narrow between them, the glacis slopes up from a blunt nose to a flat
# roof with the driver's hatch at the front left and the engine's grilles at the front right, and the
# rear is one big door. The turret (`turret`) yaws about AMV_GUN_PIVOT on a ring behind the middle, and
# its two tubes in their armoured sleeves (`gun`) pitch about the same point. The axles pair up: the
# front pair is `pivot_FL`/`pivot_FR` and `pivot_FL2`/`pivot_FR2`, which both steer, the rear pair
# `pivot_RL2`/`pivot_RR2` and `pivot_RL`/`pivot_RR`.
# --------------------------------------------------------------------------------------------------
AMV_GUN_PIVOT = (0.0, -0.7, 1.45)
AMV_RING = (0.0, -0.9, 0.925)
AMV_TUBE_X = 0.3
AMV_BARREL = 3.0


def amv_wheels(spec):
    radius, width = spec["wheel_radius"], spec["wheel_width"]
    z = radius - spec["ride"]
    names = ("F", "F2", "R2", "R")
    built = []
    for name, game_z in zip(names, spec["axles"]):
        for side, x in (("L", -spec["track"]), ("R", spec["track"])):
            tag = name[0] + side + name[1:]
            pivot = empty(f"pivot_{tag}", (x, -game_z, z))
            wheel = Part(f"wheel_{tag}", parent=pivot)
            wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=14)
            wheel.cylinder((0, 0, 0), radius * 0.55, width + 0.04, PAINT, axis="X", segments=8)
            wheel.cylinder((0, 0, 0), radius * 0.2, width + 0.08, STEEL, axis="X", segments=6)
            # Chunky tread blocks round the tyre, so that rolling can be seen.
            for k in range(6):
                a = 2 * math.pi * k / 6
                wheel.box((0, (radius + 0.015) * math.cos(a), (radius + 0.015) * math.sin(a)), (width * 0.9, 0.2, 0.06), DARK, rx=a + math.pi / 2)
            built.append(wheel.build())
    return built


def build_amv():
    spec = SPECS["amv"]
    half_x, half_y, half_z = spec["half"][0], spec["half"][2], spec["half"][1]
    body = Part("body")
    # The upper hull over the wheels: an upright nose plate, sloped glacis, flat roof, upright rear.
    upper = [(-half_y, -0.1), (half_y, -0.1), (half_y, 0.3), (2.5, half_z), (-half_y, half_z)]
    body.prism(upper, -half_x, half_x, PAINT)
    # The lower hull, narrow between the wheels, its nose sloped back under to the belly.
    lower = [(-3.6, -half_z), (3.0, -half_z), (half_y, -0.1), (-half_y, -0.1), (-half_y, -0.6)]
    body.prism(lower, -0.82, 0.82, PAINT)
    # A dark skirt under the overhang, so the wheel wells read as shadow.
    mirrored(body, 0.84, -0.15, -0.35, (0.04, 6.6, 0.5), DARK)
    # Stowage bins along the upper sides, and a rail along the roof's edge.
    for y in (1.2, -1.0, -2.6):
        mirrored(body, half_x + 0.06, y, 0.45, (0.12, 1.1, 0.5), PAINT)
    mirrored(body, half_x - 0.06, -0.6, half_z + 0.05, (0.06, 6.4, 0.06), DARK)
    # The driver's hatch and periscopes at the front left, the engine's grilles at the front right.
    body.cylinder((-0.6, 2.25, half_z + 0.04), 0.32, 0.08, DARK, segments=10)
    body.cylinder((-0.6, 2.25, half_z + 0.1), 0.26, 0.06, PAINT, segments=10)
    for x in (-0.85, -0.6, -0.35):
        body.box((x, 2.62, 0.82), (0.14, 0.06, 0.08), GLASS, rx=0.52)
    for k in range(5):
        body.box((0.65, 2.15 - k * 0.22, half_z + 0.02), (0.9, 0.1, 0.03), DARK)
    body.box((half_x + 0.02, 2.0, 0.3), (0.04, 0.8, 0.35), DARK)
    # Headlamps in guards, tow hooks and a bar across the nose.
    for side in (-1, 1):
        body.box((side * 1.05, half_y + 0.02, 0.1), (0.26, 0.05, 0.14), LAMP)
        body.box((side * 1.05, half_y + 0.06, 0.2), (0.34, 0.06, 0.04), DARK)
        body.box((side * 0.55, 3.5, -0.4), (0.12, 0.25, 0.12), DARK)
    # The rear: one big door, tail lamps and the exhaust.
    body.box((0.0, -half_y - 0.02, 0.15), (1.1, 0.05, 1.3), DARK)
    body.box((0.0, -half_y - 0.04, 0.15), (0.95, 0.04, 1.15), PAINT)
    body.box((0.3, -half_y - 0.07, 0.15), (0.12, 0.04, 0.06), STEEL)
    for side in (-1, 1):
        body.box((side * 1.15, -half_y - 0.02, 0.6), (0.14, 0.04, 0.1), RED)
    # The turret ring on the roof.
    body.cylinder(AMV_RING, 1.0, 0.1, DARK, segments=12)
    body_obj = body.build()

    # The turret yaws about the gun's pivot: a wide, flat-topped box with a chamfered face, the
    # commander's hatch, sights and smoke dischargers.
    px, py, pz = AMV_GUN_PIVOT
    turret = Part("turret", location=AMV_GUN_PIVOT, parent=body_obj)
    base, top = half_z + 0.05 - pz, half_z + 0.7 - pz
    face = [(-1.5, base), (0.55, base), (0.95, base + 0.3), (0.75, top), (-1.5, top)]
    turret.prism(face, -1.0, 1.0, PAINT)
    turret.box((0.0, -1.55, (base + top) / 2), (1.8, 0.1, top - base - 0.1), DARK)
    turret.cylinder((-0.55, -0.7, top + 0.06), 0.28, 0.12, PAINT, segments=10)
    turret.box((0.55, -0.3, top + 0.12), (0.35, 0.3, 0.24), PAINT)
    turret.box((0.55, -0.15, top + 0.14), (0.25, 0.03, 0.12), GLASS)
    for side in (-1, 1):
        for k in range(3):
            turret.cylinder((side * 1.05, 0.2 - k * 0.15, top - 0.05), 0.05, 0.25, DARK, axis="Y", segments=6)
    # The slot the tubes rise in, dark between their sleeves.
    turret.box((0.0, 0.8, (base + top) / 2 + 0.05), (1.0, 0.2, top - base - 0.1), DARK)
    turret_obj = turret.build()

    # The two tubes pitch about the pivot, each in an armoured sleeve near the breech, behind one mantlet.
    gun = Part("gun", parent=turret_obj)
    gun.box((0.0, 0.85, 0.0), (1.0, 0.25, 0.42), PAINT)
    for side in (-1, 1):
        x = side * AMV_TUBE_X
        gun.box((x, 0.6, 0.0), (0.3, 1.4, 0.3), PAINT)
        gun.cylinder((x, 1.4 + (AMV_BARREL - 1.4) / 2, 0.0), 0.085, AMV_BARREL - 1.4, DARK, axis="Y", segments=10)
        gun.cylinder((x, AMV_BARREL - 0.08, 0.0), 0.105, 0.16, DARK, axis="Y", segments=10)
    gun_obj = gun.build()
    wheels = amv_wheels(spec)
    # Head points for the seats (the crew is sealed in and never drawn): driver, operator, rider.
    add_seats([(-0.6, 2.4, 0.6), (0.0, -0.7, 0.8), (0.0, -3.0, 0.5)])
    return [body_obj, turret_obj, gun_obj, *wheels]


BUILDERS = {"amv": build_amv, "bus": build_bus, "jeep": build_jeep, "sports": build_sports, "roadster": build_roadster, "snowmobile": build_snowmobile, "golf_cart": build_golf_cart, "technical": build_technical, "snowracer": build_snowracer}


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
    # A long body is looked at from further off, in step with its length.
    far = max(1.0, SPECS[name]["half"][2] / 2.45)
    views = {
        "three": ("PERSP", Vector((6.5, 6.5, 3.2)) * far, Vector((0, 0, 0.0)), 0),
        "side": ("ORTHO", Vector((12, 0, 0.0)) * far, Vector((0, 0, 0.0)), 6.5 * far),
        "front": ("ORTHO", Vector((0, 12, 0.0)) * far, Vector((0, 0, 0.0)), 3.6 * far),
        "rear": ("PERSP", Vector((-5.5, -6.0, 3.0)) * far, Vector((0, 0, 0.0)), 0),
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
