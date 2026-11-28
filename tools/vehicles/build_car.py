"""
Builds the cars: the Bronco (the jeep), the Manta (the sports car) and the other period cars, the
thruster Roadster, the utility vehicles and the sleds, and exports them to client/public/models/.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_car.py -- \
        [--out DIR] [--only <name in BUILDERS>] [--preview DIR [--open RADIANS]]

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
      `door_FL`, `door_FR`                             a saloon's front doors (FL the driver's, -X), origin on the hinge:
                                                       the front seam, on the outer skin, at the door's foot. The game
                                                       swings them about Z (glTF Y), FL by a negative angle, FR positive
      `nozzle_rear_N`, `nozzle_lift_N`                the Roadster's thruster mouths: the rear ones blow backwards, the lift ones down
  * Materials: `paint` is tinted per car at runtime; `lamp` is drawn unlit; `dark` (tyres, trim, seats),
    `glass`, `steel` (rims, bars), `red`, `white` and `amber` keep their colours.
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
OPEN = arg("--open")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with JEEP_* and SPORTS_* in crates/shared/src/vehicles/.
# `ride` is the height of the chassis centre above flat ground once the suspension has settled
# (JEEP_RIDE_HEIGHT, SPORTS_RIDE_HEIGHT): the wheel centres sit one wheel radius above the ground.
# The ground is `ride` below the origin, so the jeep's belly is 0.44 m off it and the sports car's 0.10 m.
# --------------------------------------------------------------------------------------------------
SPECS = {
    "jeep": dict(half=(1.0, 0.42, 1.95), wheel_radius=0.55, wheel_width=0.3, track=0.9, axle=1.25, ride=0.857),
    # SPORTS_*: the Manta, low on small wheels.
    "sports": dict(half=(0.84, 0.3, 2.2), wheel_radius=0.3, wheel_width=0.2, track=0.7, axle=1.26, ride=0.417),
    # ROADSTER_*: a flat chassis on small wheels.
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
    # EPA_*: a lowered saloon on big rims and thin tyres.
    "epa": dict(half=(0.87, 0.3, 2.38), wheel_radius=0.32, wheel_width=0.22, track=0.74, axle=1.385, ride=0.447),
    # The period cars (VEHICLES-XXIII.md), kept in step with <KIND>_* in crates/shared/src/vehicles/.
    "saab_900": dict(half=(0.85, 0.32, 2.33), wheel_radius=0.31, wheel_width=0.2, track=0.72, axle=1.26, ride=0.465),
    "saab_9000": dict(half=(0.88, 0.33, 2.3), wheel_radius=0.32, wheel_width=0.21, track=0.76, axle=1.335, ride=0.475),
    "volvo_240": dict(half=(0.86, 0.36, 2.4), wheel_radius=0.32, wheel_width=0.2, track=0.73, axle=1.32, ride=0.506),
    "volvo_740": dict(half=(0.88, 0.34, 2.4), wheel_radius=0.32, wheel_width=0.2, track=0.735, axle=1.385, ride=0.485),
    "gti": dict(half=(0.84, 0.3, 2.0), wheel_radius=0.29, wheel_width=0.2, track=0.715, axle=1.235, ride=0.445),
    "transporter": dict(half=(0.92, 0.5, 2.25), wheel_radius=0.33, wheel_width=0.21, track=0.785, axle=1.23, ride=0.685),
    "kadett": dict(half=(0.83, 0.3, 2.0), wheel_radius=0.29, wheel_width=0.19, track=0.7, axle=1.26, ride=0.445),
    "omega": dict(half=(0.88, 0.33, 2.33), wheel_radius=0.31, wheel_width=0.2, track=0.725, axle=1.365, ride=0.475),
    "sierra": dict(half=(0.85, 0.32, 2.2), wheel_radius=0.3, wheel_width=0.2, track=0.725, axle=1.305, ride=0.465),
    "escort": dict(half=(0.82, 0.3, 2.0), wheel_radius=0.28, wheel_width=0.18, track=0.7, axle=1.2, ride=0.445),
    # POLICE_CAR_* and AMBULANCE_* (VEHICLES-XXV.md): the Volvo 240's and the Transporter's bodies.
    "police_car": dict(half=(0.86, 0.36, 2.4), wheel_radius=0.32, wheel_width=0.2, track=0.73, axle=1.32, ride=0.506),
    "ambulance": dict(half=(0.92, 0.5, 2.25), wheel_radius=0.33, wheel_width=0.21, track=0.785, axle=1.23, ride=0.685),
    # POLICE_VAN_* (VEHICLES-XXVI.md): the Transporter's body again.
    "police_van": dict(half=(0.92, 0.5, 2.25), wheel_radius=0.33, wheel_width=0.21, track=0.785, axle=1.23, ride=0.685),
}

MATERIAL_COLORS = {
    "paint": (0.8, 0.45, 0.1),
    "dark": (0.09, 0.09, 0.1),
    "glass": (0.55, 0.78, 0.9),
    "steel": (0.6, 0.62, 0.64),
    "lamp": (1.0, 0.96, 0.66),
    "red": (0.75, 0.05, 0.05),
    "white": (0.92, 0.92, 0.9),
    "amber": (1.0, 0.33, 0.05),
}
NAMES = list(MATERIAL_COLORS)
PAINT, DARK, GLASS, STEEL, LAMP, RED, WHITE, AMBER = range(len(NAMES))

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

    def __init__(self, name, location=(0.0, 0.0, 0.0), parent=None):
        super().__init__(name, location, parent)
        # Faces that stand alone (panels on a surface, a wheel's face), each as (points, material,
        # the way it faces): added once the solids' normals are set, wound to face that way.
        self.flat = []

    def decal(self, centre, size, normal, mat, sides=4):
        """A flat panel lying on a surface and facing along `normal`: a rectangle `size` (across, up), or
        a disc of `sides` across `size[0]`. Two triangles where a box would be twelve."""
        n = Vector(normal).normalized()
        across = (Vector((0, 0, 1)).cross(n) if abs(n.z) < 0.9 else Vector((1, 0, 0))).normalized()
        up = n.cross(across).normalized()
        c = Vector(centre)
        if sides == 4:
            points = [c + across * (sx * size[0] / 2) + up * (sz * size[1] / 2) for sx, sz in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        else:
            points = [c + (across * math.cos(2 * math.pi * k / sides) + up * math.sin(2 * math.pi * k / sides)) * size[0] / 2 for k in range(sides)]
        self.flat.append(([tuple(p) for p in points], mat, tuple(n)))

    def build(self):
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts, dist=1e-4)
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces)
        for points, mat, normal in self.flat:
            face = self.bm.faces.new([self.bm.verts.new(p) for p in points])
            face.material_index = mat
            face.normal_update()
            if face.normal.dot(Vector(normal)) < 0:
                face.normal_flip()
        mesh = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for m in MATERIALS:
            mesh.materials.append(m)
        obj = bpy.data.objects.new(self.name, mesh)
        obj.location = self.location
        scene.collection.objects.link(obj)
        if self.parent:
            obj.parent = self.parent
        return obj

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


class Door(Part):
    """A front door of a saloon (`door_FL` on the driver's side, -X, or `door_FR`), cut out of the
    body's side and built in the car's frame like the body. `close` turns the outer skin built so far
    into a panel `THICK` deep, dark on the inside, so an opened door is not seen through; whatever is
    added after it (trim, seams, the handle, lettering) lies on that panel. `build` moves the door's
    origin to its hinge, where the game swings it open about the vertical."""

    # How deep the door is between its outer skin and the dark inner one, metres.
    THICK = 0.03

    def __init__(self, name, side):
        super().__init__(name)
        # -1 for the left door, 1 for the right: the way its outer skin faces along X.
        self.side = side
        # The side glass and the posts painted into it, (points, material), part of the outer skin.
        self.skin = []
        self.panel = set()

    def close(self):
        """Gives the outer skin built so far an inner skin `THICK` inside it, dark where the outer one
        is painted and glass where it is glass, and joins the two round every open edge in dark."""
        bm = self.bm
        for points, mat in self.skin:
            bm.faces.new([bm.verts.new(p) for p in points]).material_index = mat
        bmesh.ops.remove_doubles(bm, verts=bm.verts, dist=1e-4)
        bmesh.ops.delete(bm, geom=[v for v in bm.verts if not v.link_faces], context="VERTS")
        outer = list(bm.faces)
        for f in outer:
            f.normal_update()
            if f.normal.x * self.side < 0:
                f.normal_flip()
        edges = [(e, e.link_faces[0]) for e in bm.edges if len(e.link_faces) == 1]
        inward = Vector((-self.side * self.THICK, 0.0, 0.0))
        inner = {v: bm.verts.new(v.co + inward) for v in list(bm.verts)}
        for f in outer:
            g = bm.faces.new([inner[v] for v in reversed(f.verts)])
            g.material_index = GLASS if f.material_index == GLASS else DARK
        for e, f in edges:
            v0, v1 = e.verts
            rim = bm.faces.new((v0, v1, inner[v1], inner[v0]))
            rim.material_index = DARK
            rim.normal_update()
            if rim.normal.dot((v0.co + v1.co) / 2 - f.calc_center_median()) < 0:
                rim.normal_flip()
        self.panel = set(bm.faces)

    def build(self, hinge):
        """The door's object, its origin at `hinge` (the car's frame) and its mesh about it."""
        bm = self.bm
        bmesh.ops.recalc_face_normals(bm, faces=[f for f in bm.faces if f not in self.panel])
        for points, mat, normal in self.flat:
            face = bm.faces.new([bm.verts.new(p) for p in points])
            face.material_index = mat
            face.normal_update()
            if face.normal.dot(Vector(normal)) < 0:
                face.normal_flip()
        bmesh.ops.translate(bm, vec=-Vector(hinge), verts=bm.verts)
        mesh = bpy.data.meshes.new(self.name)
        bm.to_mesh(mesh)
        bm.free()
        for m in MATERIALS:
            mesh.materials.append(m)
        obj = bpy.data.objects.new(self.name, mesh)
        obj.location = hinge
        scene.collection.objects.link(obj)
        return obj


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


# --------------------------------------------------------------------------------------------------
# The EPA traktor: an old boxy saloon cut down into a pickup. A wedge of a bonnet, the front doors
# and their windows, then a blank panel where the back doors were welded shut and a wall with a
# window and a slow-vehicle triangle behind the seats. The boot and the back of the cabin are one
# short open bed, with tall tail lamps either side of the plate and a tow ball under the bumper.
# Black bumpers and side strips, a sunroof, and big rims on thin tyres.
# --------------------------------------------------------------------------------------------------
def triangle(part, centre, side, depth, mat):
    """An upright triangle, point up, facing along Y: a slow-vehicle sign."""
    cx, cy, cz = centre
    h = side * math.sqrt(3) / 2
    outline = [(cx - side / 2, cz - h / 3), (cx + side / 2, cz - h / 3), (cx, cz + 2 * h / 3)]
    a = [part.bm.verts.new((x, cy - depth / 2, z)) for x, z in outline]
    b = [part.bm.verts.new((x, cy + depth / 2, z)) for x, z in outline]
    faces = [part.bm.faces.new(a), part.bm.faces.new(b[::-1])]
    for i in range(3):
        j = (i + 1) % 3
        faces.append(part.bm.faces.new((a[i], a[j], b[j], b[i])))
    for face in faces:
        face.material_index = mat


def build_epa():
    spec = SPECS["epa"]
    hx, hy = 0.86, spec["half"][2]
    axle, arch = spec["axle"], 0.4
    # Heights over the origin: the bottom of the sills, the top of the wheel arches, the waist line
    # under the windows, and the roof.
    sill, top, belt, roof = -0.22, 0.25, 0.45, 0.93
    # Along the car: the foot of the windscreen, its top, the door's back edge and the cab's back wall.
    screen, crown, door, wall = 0.9, 0.2, -0.22, -0.78
    body = Part("body")
    # The lower body, cut away round the wheels, with the wells dark between them.
    for y0, y1 in ((-hy, -axle - arch), (-axle + arch, axle - arch), (axle + arch, hy)):
        body.box((0.0, (y0 + y1) / 2, (sill + top) / 2), (2 * hx, y1 - y0, top - sill), PAINT)
    for y in (-axle, axle):
        body.box((0.0, y, (sill + top) / 2), (2 * (spec["track"] - spec["wheel_width"] / 2) - 0.02, 2 * arch, top - sill), DARK)
    # The bonnet and the wings, a wedge that rises to the windscreen.
    body.prism([(hy - 0.02, top), (hy - 0.02, 0.33), (screen, belt), (screen, top)], -hx, hx, PAINT)
    # The doors and the welded back doors up to the waist, a dash and two seats inside.
    mirrored(body, hx - 0.04, (screen + wall) / 2, (top + belt) / 2, (0.08, screen - wall, belt - top), PAINT)
    body.box((0.0, screen - 0.2, belt - 0.02), (2 * hx - 0.16, 0.4, 0.16), DARK)
    for side in (-1, 1):
        body.box((side * 0.38, -0.1, top + 0.06), (0.5, 0.5, 0.12), DARK)
        body.box((side * 0.38, -0.38, 0.55), (0.5, 0.1, 0.6), DARK, rx=-0.15)
    body.cylinder((-0.38, 0.45, 0.6), 0.18, 0.03, DARK, axis="Y", segments=12)
    # The windscreen and its posts, the roof with a sunroof, and the side glass up to the B post.
    rake = -math.atan2(roof - belt, screen - crown)
    rise = math.hypot(roof - belt, screen - crown)
    body.box((0.0, (screen + crown) / 2, (belt + roof) / 2), (1.5, rise, 0.03), GLASS, rx=rake)
    mirrored(body, 0.78, (screen + crown) / 2, (belt + roof) / 2, (0.07, rise, 0.07), PAINT, rx=rake)
    body.box((0.0, (crown + wall) / 2, roof - 0.03), (1.62, crown - wall + 0.04, 0.06), PAINT)
    body.box((0.0, -0.2, roof + 0.015), (0.7, 0.5, 0.03), DARK)
    for side in (-1, 1):
        x = side * 0.79
        body.prism([(screen - 0.04, belt), (crown + 0.02, roof - 0.06), (door, roof - 0.06), (door, belt)], x - 0.01, x + 0.01, GLASS)
        # The B post, and the blank panel where the back door's window was.
        body.box((x, door - 0.04, (belt + roof) / 2), (0.05, 0.08, roof - belt), PAINT)
        body.box((x, (door + wall) / 2, (belt + roof) / 2), (0.04, door - wall, roof - belt), PAINT)
    # The cab's back wall: paint round a big window, and the slow-vehicle triangle in it.
    body.box((0.0, wall, (top + belt) / 2), (2 * hx - 0.08, 0.06, belt - top), PAINT)
    mirrored(body, 0.71, wall, (belt + roof) / 2, (0.18, 0.06, roof - belt), PAINT)
    body.box((0.0, wall, belt + 0.03), (1.3, 0.06, 0.06), PAINT)
    body.box((0.0, wall, (belt + roof) / 2 + 0.01), (1.26, 0.03, roof - belt - 0.06), GLASS)
    triangle(body, (-0.2, wall - 0.03, belt + 0.22), 0.36, 0.01, AMBER)
    # The bed: its floor over the rear axle, its sides and the tailgate.
    body.box((0.0, (wall - hy) / 2, top + 0.015), (2 * hx - 0.08, wall + hy, 0.03), DARK)
    mirrored(body, hx - 0.04, (wall - hy) / 2, (top + belt) / 2, (0.08, wall + hy, belt - top), PAINT)
    body.box((0.0, -hy + 0.04, (top + belt) / 2), (2 * hx, 0.08, belt - top), PAINT)
    # A fuel flap on the left of the bed.
    body.box((-hx - 0.003, -1.0, 0.35), (0.01, 0.14, 0.12), WHITE)
    # The nose: a black bumper, a grille in a bright surround, square headlamps and amber indicators.
    body.box((0.0, hy + 0.03, -0.11), (2 * hx + 0.04, 0.16, 0.2), DARK)
    mirrored(body, hx - 0.02, hy - 0.2, -0.11, (0.08, 0.36, 0.2), DARK)
    body.box((0.0, hy - 0.01, 0.13), (0.66, 0.03, 0.24), STEEL)
    body.box((0.0, hy, 0.13), (0.58, 0.03, 0.18), DARK)
    mirrored(body, 0.55, hy - 0.01, 0.14, (0.36, 0.04, 0.16), LAMP)
    mirrored(body, 0.8, hy - 0.02, 0.14, (0.1, 0.04, 0.16), AMBER)
    # Black strips down the sides, door seams, handles and mirrors.
    mirrored(body, hx + 0.01, 0.0, 0.05, (0.02, 2 * (axle - arch) - 0.1, 0.06), DARK)
    for y in (screen, door):
        mirrored(body, hx + 0.002, y, (sill + belt) / 2 + 0.05, (0.01, 0.012, belt - sill - 0.1), DARK)
    mirrored(body, hx + 0.01, door + 0.15, belt - 0.07, (0.02, 0.12, 0.03), DARK)
    mirrored(body, hx + 0.08, screen - 0.1, belt + 0.1, (0.12, 0.07, 0.1), DARK)
    # The tail: tall lamps either side of the plate, the bumper, a tow ball and the exhaust.
    for side in (-1, 1):
        x = side * (hx - 0.11)
        body.box((x, -hy - 0.005, 0.2), (0.2, 0.03, 0.4), RED)
        body.box((x, -hy - 0.015, 0.2), (0.2, 0.02, 0.08), WHITE)
        body.box((x, -hy - 0.015, 0.12), (0.2, 0.02, 0.05), AMBER)
    body.box((0.0, -hy - 0.005, 0.22), (0.6, 0.02, 0.17), DARK)
    body.box((0.0, -hy - 0.015, 0.22), (0.52, 0.02, 0.11), WHITE)
    body.box((0.0, -hy - 0.04, -0.11), (2 * hx + 0.04, 0.16, 0.22), DARK)
    mirrored(body, hx - 0.02, -hy + 0.2, -0.11, (0.08, 0.36, 0.22), DARK)
    body.box((0.0, -hy - 0.16, -0.24), (0.06, 0.26, 0.05), DARK)
    body.box((0.0, -hy - 0.27, -0.19), (0.03, 0.03, 0.08), STEEL)
    body.cylinder((0.0, -hy - 0.27, -0.13), 0.045, 0.06, STEEL, segments=8)
    body.cylinder((-0.45, -hy - 0.02, -0.24), 0.035, 0.2, DARK, axis="Y", segments=6)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.7)
    # EPA_SEAT_HEADS, in this frame: x, -z, y.
    add_seats([(-0.38, -0.1, 0.75), (0.38, -0.1, 0.75)])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The period cars, 1980 to 1995 (VEHICLES-XXIII.md): one body builder, `build_saloon`, drawn from a
# side profile and a glasshouse, and each car's own shape and details on top of it.
# --------------------------------------------------------------------------------------------------
# Each period car's <KIND>_SEAT_HEADS, in the game's hull frame (x, y, z). Keep in step with crates/shared/src/vehicles/.
HEADS = {
    "saab_900": [(-0.38, 0.71, 0.0), (0.38, 0.71, 0.0), (-0.4, 0.69, 0.88), (0.4, 0.69, 0.88)],
    "saab_9000": [(-0.38, 0.7, 0.0), (0.38, 0.7, 0.0), (-0.4, 0.68, 0.88), (0.4, 0.68, 0.88), (0.0, 0.68, 0.88)],
    "volvo_240": [(-0.38, 0.67, 0.0), (0.38, 0.67, 0.0), (-0.4, 0.65, 0.88), (0.4, 0.65, 0.88), (0.0, 0.65, 0.88)],
    "volvo_740": [(-0.38, 0.69, 0.0), (0.38, 0.69, 0.0), (-0.4, 0.67, 0.88), (0.4, 0.67, 0.88), (0.0, 0.67, 0.88)],
    "gti": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75)],
    "transporter": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
    "kadett": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75), (0.0, 0.71, 0.75)],
    "omega": [(-0.38, 0.7, 0.0), (0.38, 0.7, 0.0), (-0.4, 0.68, 0.88), (0.4, 0.68, 0.88), (0.0, 0.68, 0.88)],
    "sierra": [(-0.38, 0.71, 0.0), (0.38, 0.71, 0.0), (-0.4, 0.69, 0.88), (0.4, 0.69, 0.88), (0.0, 0.69, 0.88)],
    "escort": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75), (0.0, 0.71, 0.75)],
    "sports": [(-0.37, 0.72, 0.1), (0.37, 0.72, 0.1), (-0.38, 0.66, 0.85), (0.38, 0.66, 0.85)],
    "police_car": [(-0.38, 0.67, 0.0), (0.38, 0.67, 0.0), (-0.4, 0.65, 0.88), (0.4, 0.65, 0.88), (0.0, 0.65, 0.88)],
    "ambulance": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
    "police_van": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
}


def hexa(part, corners, mat):
    """A six-faced solid from its eight corners: four round the bottom, then the four above them in the same order."""
    verts = [part.bm.verts.new(c) for c in corners]
    bottom, top = verts[:4], verts[4:]
    faces = [part.bm.faces.new(bottom[::-1]), part.bm.faces.new(top)]
    for i in range(4):
        j = (i + 1) % 4
        faces.append(part.bm.faces.new((bottom[i], bottom[j], top[j], top[i])))
    for face in faces:
        face.material_index = mat


def beam(part, p0, p1, thick, mat, broad=None):
    """A bar from point p0 to point p1, `thick` across and `broad` (as thick, unless given) the other way
    round: a window post, a roll bar. For a bar in a side of the car, `thick` is across the car."""
    a, b = Vector(p0), Vector(p1)
    along = (b - a).normalized()
    u = along.cross(Vector((0, 0, 1)) if abs(along.z) < 0.9 else Vector((1, 0, 0))).normalized() * thick / 2
    v = along.cross(u).normalized() * (thick if broad is None else broad) / 2
    ring = [u + v, -u + v, -u - v, u - v]
    hexa(part, [tuple(a + r) for r in ring] + [tuple(b + r) for r in ring], mat)


def clip(poly, keep):
    """The convex outline `poly` (of (y, z) or (x, y, z) points) cut down to where the linear `keep` is
    not negative."""
    out = []
    for i, p in enumerate(poly):
        q = poly[(i + 1) % len(poly)]
        fp, fq = keep(p), keep(q)
        if fp >= 0:
            out.append(p)
        if (fp >= 0) != (fq >= 0):
            t = fp / (fp - fq)
            out.append(tuple(u + t * (v - u) for u, v in zip(p, q)))
    return out


# How far an arch's edge falls for each metre it is from the axle.
ARCH_FALL = 0.45


def round_wheels(body, spec, profile, x0, x1, mat, arch_top):
    """Extrudes the side `profile` from x0 to x1, leaving room over each wheel up to `arch_top`."""
    axle, arch = spec["axle"], spec["wheel_radius"] + 0.08
    # Each arch is cut highest over the axle and falls away to either side, in two halves.
    cuts = [(-9.0, -axle - arch, None), (-axle - arch, -axle, -axle), (-axle, -axle + arch, -axle), (-axle + arch, axle - arch, None), (axle - arch, axle, axle), (axle, axle + arch, axle), (axle + arch, 9.0, None)]
    for y0, y1, over in cuts:
        poly = clip(clip(profile, lambda p: p[0] - y0), lambda p: y1 - p[0])
        if over is not None:
            fall = ARCH_FALL if y0 >= over else -ARCH_FALL
            poly = clip(poly, lambda p: p[1] - arch_top + fall * (p[0] - over))
        if len(poly) >= 3:
            body.prism(poly, x0, x1, mat)


def wheel_wells(body, spec, sill, arch_top):
    """The dark wells the wheels turn in, inside the body between them."""
    inner = spec["track"] - spec["wheel_width"] / 2 - 0.02
    for y in (-spec["axle"], spec["axle"]):
        body.box((0.0, y, (sill + arch_top) / 2), (2 * inner, 2 * (spec["wheel_radius"] + 0.08), arch_top - sill), DARK)


def seats_for(body, heads, wheel=True):
    """A cushion and a back under each rider's head (Blender frame), and the driver's steering wheel."""
    for x, y, z in heads:
        body.box((x, y + 0.05, z - 0.7), (0.46, 0.48, 0.12), DARK)
        body.box((x, y - 0.22, z - 0.38), (0.46, 0.1, 0.56), DARK, rx=-0.12)
    if wheel:
        x, y, z = heads[0]
        body.cylinder((x, y + 0.45, z - 0.3), 0.18, 0.03, DARK, axis="Y", segments=12)


def cabin(body, heads):
    """The seats under the riders' heads (Blender frame): one each in the front row, a bench across
    each row behind it, and the driver's steering wheel."""
    rows = {}
    for x, y, z in heads:
        rows.setdefault(round(y, 2), []).append((x, z))
    for k, (y, row) in enumerate(sorted(rows.items(), reverse=True)):
        h = min(z for _, z in row)
        if k == 0:
            for x, _ in row:
                body.box((x, y + 0.05, h - 0.7), (0.46, 0.48, 0.12), DARK)
                body.box((x, y - 0.22, h - 0.38), (0.46, 0.1, 0.56), DARK, rx=-0.12)
        else:
            xs = [x for x, _ in row]
            span = max(xs) - min(xs) + 0.5
            body.box(((max(xs) + min(xs)) / 2, y + 0.05, h - 0.7), (span, 0.48, 0.12), DARK)
            body.box(((max(xs) + min(xs)) / 2, y - 0.22, h - 0.38), (span, 0.1, 0.56), DARK, rx=-0.12)
    x, y, z = heads[0]
    body.cylinder((x, y + 0.45, z - 0.3), 0.18, 0.03, DARK, axis="Y", segments=8)


def mirrored_decal(body, x, y, z, size, normal, mat):
    """The same panel on both sides of the car, at +/-x, each facing out (or both along `normal` when it faces along the car)."""
    for side in (-1, 1):
        n = (side * normal[0], normal[1], normal[2])
        body.decal((side * x, y, z), size, n, mat)


def to_blender(heads):
    """SEAT_HEADS in the game's hull frame, (x, y, z), as Blender points: (x, -z, y)."""
    return [(x, -z, y) for x, y, z in heads]


def obox(part, centre, size, mat, rz=0.0, rx=0.0):
    """A box turned `rz` about the vertical, then `rx` about its own across-axis."""
    m = Matrix.Translation(centre) @ Matrix.Rotation(rz, 4, "Z") @ Matrix.Rotation(rx, 4, "X") @ Matrix.Diagonal((*size, 1.0))
    part._tag(bmesh.ops.create_cube(part.bm, size=1.0, matrix=m)["verts"], mat)


def shell(part, rings, mat_of, cap=None):
    """A skin through closed rings of points, each ring with as many points: the face between ring `s`
    and the next along edge `k` of the ring (point `k` to the next) is `mat_of(s, k)`. With `cap`, the
    first and last rings are closed in that material. A face of material None is one nobody sees,
    inside the body, and is left out."""
    verts = [[part.bm.verts.new(p) for p in ring] for ring in rings]
    n = len(verts[0])
    for s, (a, b) in enumerate(zip(verts, verts[1:])):
        for k in range(n):
            mat = mat_of(s, k)
            if mat is None:
                continue
            j = (k + 1) % n
            part.bm.faces.new((a[k], a[j], b[j], b[k])).material_index = mat
    if cap is not None:
        part.bm.faces.new(verts[0]).material_index = cap
        part.bm.faces.new(verts[-1][::-1]).material_index = cap


def along_line(points, a):
    """The height of the line through `points`, (along, height) from the front back, at `along`."""
    if a <= points[0][0]:
        return points[0][1]
    for (a0, h0), (a1, h1) in zip(points, points[1:]):
        if a <= a1:
            return h0 + (h1 - h0) * (a - a0) / max(a1 - a0, 1e-6)
    return points[-1][1]


def wheel_face(part, x, out, radii, segments, pattern):
    """The outer face of a wheel, `x` along its axle facing `out`: rings at `radii` round its centre,
    each sector of ring `r` (0 the innermost) in material `pattern(r, k)`. The spokes, holes and slots
    are colours on it, not solids, so a wheel face is a few dozen triangles however many spokes it has."""
    ring = lambda radius: [(x, radius * math.cos(2 * math.pi * k / segments), radius * math.sin(2 * math.pi * k / segments)) for k in range(segments)]
    rings = [ring(r) for r in radii]
    hub = (x + out * 0.006, 0.0, 0.0)
    normal = (float(out), 0.0, 0.0)
    for k in range(segments):
        j = (k + 1) % segments
        part.flat.append(([hub, rings[0][k], rings[0][j]], pattern(0, k), normal))
        for r in range(1, len(radii)):
            part.flat.append(([rings[r - 1][k], rings[r][k], rings[r][j], rings[r - 1][j]], pattern(r, k), normal))


def styled_wheels(spec, style, rim_ratio=0.62):
    """Four wheels on steering pivots, each a tyre with its face in `style`: 'steel' (a black steel
    wheel under a hubcap with slots), 'dial' (a flat disc with round holes) or 'spokes3', 'spokes5' and
    'spokes6' (an alloy with that many spokes)."""
    radius, width = spec["wheel_radius"], spec["wheel_width"]
    z = radius - spec["ride"]
    rim = radius * rim_ratio
    built = []
    for fore, y in (("F", spec["axle"]), ("R", -spec["axle"])):
        for side, x in (("L", -spec["track"]), ("R", spec["track"])):
            tag, out = fore + side, (-1 if side == "L" else 1)
            pivot = empty(f"pivot_{tag}", (x, y, z))
            wheel = Part(f"wheel_{tag}", parent=pivot)
            wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=12)
            face = out * (width / 2 + 0.004)
            if style == "steel":
                wheel_face(wheel, face, out, (rim * 0.78, rim), 12, lambda r, k: (DARK if k % 3 == 0 else STEEL) if r == 0 else DARK)
            elif style == "dial":
                wheel_face(wheel, face, out, (rim * 0.3, rim * 0.82, rim), 12, lambda r, k: DARK if r == 1 and k % 2 == 0 else STEEL)
            else:
                spokes = int(style[len("spokes"):])
                every = 12 // spokes
                wheel_face(wheel, face, out, (rim * 0.28, rim * 0.86, rim), 12, lambda r, k: STEEL if r != 1 or k % every == 0 else DARK)
            built.append(wheel.build())
    return built


def wrap(part, spec, end, lo, hi, out, deep, mat, nr, back=0.3):
    """A bumper round the front (`end` 1) or the back (-1) of the car, from `lo` to `hi` (Blender z),
    standing `out` proud of the body and `deep` into it, round its corners (radius `nr`) and back along
    its sides by `back`."""
    hx, hz = spec["half"][0], spec["half"][2]
    right = [(hx, hz - nr - back, 1.0, 0.0)]
    for deg in (0, 45, 90):
        t = math.radians(deg)
        right.append((hx - nr + nr * math.cos(t), hz - nr + nr * math.sin(t), math.cos(t), math.sin(t)))
    path = right + [(-x, y, -nx, ny) for x, y, nx, ny in reversed(right)]
    rings = []
    for x, y, nx, ny in path:
        ring = [(x - nx * deep, y - ny * deep, lo), (x + nx * out, y + ny * out, lo), (x + nx * out, y + ny * out, hi), (x - nx * deep, y - ny * deep, hi)]
        rings.append([(px, end * py, pz) for px, py, pz in ring])
    shell(part, rings, lambda s, k: None if k == 3 else mat, cap=mat)


def build_saloon(name, c, heads, details=None):
    """A closed car of the 1980s, from `c`: heights above the ground and lengths back from the front
    face, metres. `top` is the line along the top of the lower body (bonnet, waist, boot or tail) and
    `glass` the line along the top of the glasshouse (windscreen, roof, back glass), both as (along,
    height) from front to back. The body is lofted through cross-sections: rounded at the corners in
    plan (`nose_round`, `tail_round`), its sides tucked in at the sill and leaning in from a shoulder
    crease, and cut round the wheels in arches. `heads` is the car's SEAT_HEADS, and `details` draws
    whatever is its own on top, given the measurements worked out here."""
    spec = SPECS[name]
    hx, hz = spec["half"][0], spec["half"][2]
    length, ground = 2 * hz, -spec["ride"]
    radius, axle = spec["wheel_radius"], spec["axle"]
    z = lambda h: h + ground
    y_of = lambda a: hz - a
    nr, tr = c["nose_round"], c["tail_round"]
    sill = c["sill"]
    drop, inset = c.get("shoulder", (0.09, 0.05))
    tuck = c.get("tuck", 0.05)
    upper = c.get("upper", PAINT)
    clad = c.get("clad")

    def width(a):
        """Half the body's width `a` back from the front face, rounded at both ends."""
        if a < nr:
            d = (nr - a) / nr
            return hx - nr + nr * math.sqrt(max(0.0, 1 - d * d))
        if a > length - tr:
            d = (a - (length - tr)) / tr
            return hx - tr + tr * math.sqrt(max(0.0, 1 - d * d))
        return hx

    arch_r = radius + 0.06
    centre_h = radius
    arches = [hz - axle, hz + axle]

    def bottom(a):
        """The underside of the body's side at `a`: the sill, rising under the bumpers and over the wheels."""
        h = sill
        front, back = c.get("chin", (sill, 0.3)), c.get("tail_lift", (sill, 0.3))
        if a < front[1]:
            h = max(h, front[0] + (sill - front[0]) * a / front[1])
        if a > length - back[1]:
            h = max(h, back[0] + (sill - back[0]) * (length - a) / back[1])
        for at in arches:
            if abs(a - at) <= arch_r + 1e-6:
                h = max(h, centre_h + math.sqrt(max(0.0, arch_r * arch_r - (a - at) ** 2)))
        return h

    # Where the cross-sections are: round the corners, at every bend of the top line, and round each arch.
    stations = {0.0, length}
    t = math.radians(45)
    stations |= {nr * (1 - math.cos(t)), length - tr * (1 - math.cos(t))}
    stations |= {nr, length - tr, c.get("chin", (0, 0.3))[1], length - c.get("tail_lift", (0, 0.3))[1]}
    stations |= {a for a, _ in c["top"] if 0 < a < length}
    for at in arches:
        stations |= {at - arch_r - 0.004, at + arch_r + 0.004}
        stations |= {at - arch_r * math.cos(math.radians(deg)) for deg in (0, 45, 90, 135, 180)}
    # The front door's seams are stations of their own, so its faces end exactly on them; a station
    # close to a seam gives way to it.
    d0, d1 = c["doors"][0], c["doors"][1]
    seams = lambda points: sorted({a for a in points if min(abs(a - d0), abs(a - d1)) > 0.003} | {d0, d1})
    stations = seams(stations)
    merged = []
    for a in stations:
        if not merged or a - merged[-1] > 0.002:
            merged.append(a)
    in_door = lambda a0, a1: a0 >= d0 - 1e-6 and a1 <= d1 + 1e-6

    def side_of(a):
        """The right half of the cross-section `a` back from the front face, bottom to top, as (out,
        height, the material of the face up to the next point)."""
        w, hb, ht = width(a), bottom(a), along_line(c["top"], a)
        hs = max(ht - drop, hb + 0.6 * (ht - hb))
        hl = hb + min(0.07, (hs - hb) * 0.3)
        lower = clad[1] if clad else PAINT
        half = [(w - tuck, hb, lower), (w, hl, lower)]
        if clad:
            half.append((w, min(max(clad[0], hl + 0.01), hs - 0.01), PAINT))
        return half + [(w, hs, PAINT), (w - inset, ht, PAINT)]

    rings, mats = [], []
    for a in merged:
        hb, ht = bottom(a), along_line(c["top"], a)
        half = side_of(a)
        ring = [(0.0, hb)] + [(x, h) for x, h, _ in half] + [(0.0, ht)] + [(-x, h) for x, h, _ in half[::-1]]
        mats = [DARK] + [m for _, _, m in half[:-1]] + [PAINT, PAINT] + [m for _, _, m in half[-2::-1]] + [DARK]
        rings.append([(x, y_of(a), z(h)) for x, h in ring])
    body = Part("body")
    # The top of the lower body under the glasshouse is not seen.
    glass = c["glass"]
    under = lambda s: merged[s] >= glass[0][0] + 0.01 and merged[s + 1] <= glass[-1][0] - 0.01
    top_edges = {len(half), len(half) + 1}
    # The front doors: each side's faces between the seams, from the sill to the shoulder, cut out of
    # the body into a door of its own. The left side is the driver's (-X).
    H = len(half)
    sides = {1: set(range(1, H)), -1: set(range(H + 2, 2 * H + 1))}
    doors = {-1: Door("door_FL", -1), 1: Door("door_FR", 1)}
    door_face = lambda s, k: in_door(merged[s], merged[s + 1]) and next((sx for sx, ks in sides.items() if k in ks), None)
    shell(body, rings, lambda s, k: None if (k in top_edges and under(s)) or door_face(s, k) else mats[k], cap=PAINT)
    for sx, door in doors.items():
        shell(door, rings, lambda s, k: mats[k] if door_face(s, k) == sx else None)
    # The dark wells the wheels turn in.
    inner = spec["track"] - spec["wheel_width"] / 2 - 0.02
    for at in arches:
        body.box((0.0, y_of(at), z((sill + centre_h + arch_r) / 2)), (2 * inner, 2 * arch_r, centre_h + arch_r - sill), DARK)

    # The glasshouse, lofted along the glass line and leaning in towards the top (`tumble`). Each
    # cross-section is a side up to the top of the side glass, a band round the corner as broad as an A
    # post (`a_post`), and a top across. The band is painted all along, so it is the A posts up the
    # windscreen's edges, the roof's edges and the rear posts down the back glass's, all flush with the
    # glass. The top is the windscreen, the roof and the back glass. The windscreen stops where the line
    # flattens into the roof, so the curve over the top of it is painted.
    roof_h = max(h for _, h in glass)
    tumble = c.get("tumble", 0.12)
    pw = c.get("a_post", 0.07)
    gstations = seams({a for a, _ in glass})
    grings = []
    for a in gstations:
        wb = width(a) - inset - 0.005
        hb = along_line(c["top"], a)
        hg = max(along_line(glass, a), hb + 0.02)
        f = (hg - hb) / max(roof_h - hb, 0.01)
        wt = wb - tumble * f
        e = min(0.05, (hg - hb) / 2)
        half = [(wb, hb - 0.01), (wt, hg - e), (wt - pw, hg)]
        grings.append([(x, y_of(a), z(h)) for x, h in [(0.0, hb - 0.01)] + half + [(0.0, hg)] + [(-x, h) for x, h in half[::-1]]])
    screen = [a for a, h in glass if a <= next(a for a, h in glass if h > roof_h - 0.025)]
    back = [a for a, h in glass if a >= [a for a, h in glass if h > roof_h - 0.025][-1]]
    screen_top = next((a0 for (a0, h0), (a1, h1) in zip(glass, glass[1:]) if (h1 - h0) < 0.6 * (a1 - a0)), screen[-1])
    roofed = lambda s: gstations[s] >= screen_top - 1e-6 and gstations[s + 1] <= back[0] + 1e-6
    shell(body, grings, lambda s, k: None if k in (0, 1, 6, 7) else upper if k in (2, 5) or roofed(s) else GLASS, cap=GLASS)

    # The sides: glass, with the other posts painted into it as seen from the side. A B post is a band
    # `b_post` broad round each of `posts`; the rear post is everything behind a line `c_post` ahead of
    # the back glass and running down with it.
    def ahead(a0, h0, a1, h1):
        """How far a point is ahead of the line through (a0, h0) and (a1, h1) seen from the side, metres."""
        return lambda p: a0 + (a1 - a0) * (p[2] - ground - h0) / (h1 - h0) - (hz - p[1])
    behind = lambda f: (lambda p: -f(p))
    bw = c.get("b_post", 0.08)
    regions = [([ahead(a + bw / 2, 0.0, a + bw / 2, 1.0), behind(ahead(a - bw / 2, 0.0, a - bw / 2, 1.0))], c.get("b_mat", upper)) for a in c.get("posts", ())]
    cp = c.get("c_post", 0.1)
    regions.append(([behind(ahead(back[0] - cp, roof_h, back[-1] - cp, along_line(c["top"], back[-1])))], upper))

    def area(poly):
        n = Vector()
        for p, q in zip(poly, poly[1:] + poly[:1]):
            n += Vector(p).cross(Vector(q))
        return n.length / 2

    for s, (ra, rb) in enumerate(zip(grings, grings[1:])):
        for i, j, out in ((1, 2, 1.0), (6, 7, -1.0)):
            pieces = [([ra[i], ra[j], rb[j], rb[i]], GLASS)]
            for keeps, mat in regions:
                split = []
                for poly, m in pieces:
                    if m != GLASS:
                        split.append((poly, m))
                        continue
                    for keep in keeps:
                        split.append((clip(poly, behind(keep)), GLASS))
                        poly = clip(poly, keep)
                        if len(poly) < 3:
                            break
                    split.append((poly, mat))
                pieces = [(poly, m) for poly, m in split if len(poly) >= 3 and area(poly) > 1e-8]
            # They meet their neighbours only at points along an edge, so they are faced out by hand.
            if in_door(gstations[s], gstations[s + 1]):
                doors[int(out)].skin += pieces
            else:
                body.flat += [(poly, m, (out, 0.0, 0.0)) for poly, m in pieces]
    # The doors' tops are the tops of their side glass; the posts' band over them stays on the body.
    door_top = max(r[2][2] for a, r in zip(gstations, grings) if d0 - 1e-6 <= a <= d1 + 1e-6)
    door_foot = z(min(bottom(a) for a in merged if d0 - 1e-6 <= a <= d1 + 1e-6))
    for door in doors.values():
        door.close()

    for sx in (1, -1):
        # The trim along the foot of the side windows, the piece on the front door with it.
        a0, a1 = screen[0] + 0.05, back[-1] - 0.05
        for p0, p1, part in ((a0, min(a1, d0), body), (max(a0, d0), min(a1, d1), doors[sx]), (max(a0, d1), a1, body)):
            if p1 - p0 > 0.01:
                hb0, hb1 = along_line(c["top"], p0), along_line(c["top"], p1)
                beam(part, (sx * (width(p0) - inset + 0.004), y_of(p0), z(hb0) + 0.012), (sx * (width(p1) - inset + 0.004), y_of(p1), z(hb1) + 0.012), 0.02, c.get("trim", DARK), 0.025)

    # Inside: a dash under the windscreen, the seats and the wheel.
    hd = along_line(c["top"], screen[0])
    body.box((0.0, y_of(screen[0]) - 0.22, z(hd) - 0.06), (2 * hx - 0.24, 0.36, 0.14), DARK)
    cabin(body, to_blender(heads))
    # Wipers lying at the foot of the windscreen.
    a = screen[0] + 0.07
    hw = along_line(glass, a) + 0.012
    for x0, x1 in ((-0.62, -0.08), (-0.02, 0.5)):
        beam(body, (x0, y_of(a + 0.02), z(hw + 0.02)), (x1, y_of(a), z(hw)), 0.015, DARK)

    # Bumpers, wrapped round the corners, with the plates on them.
    bm = c["bumpers"]
    wrap(body, spec, 1, z(bm["lo"]), z(bm["hi"]), bm["out"], 0.1, bm["mat"], nr, bm.get("back", 0.3))
    wrap(body, spec, -1, z(bm["lo"]), z(bm["hi"]), bm["out"], 0.1, bm["mat"], tr, bm.get("back", 0.3))
    FRONT, BACK = (0, 1, 0), (0, -1, 0)
    body.decal((0.0, hz + bm["out"] + 0.003, z((bm["lo"] + bm["hi"]) / 2)), (0.5, min(0.11, bm["hi"] - bm["lo"] - 0.03)), FRONT, WHITE)

    # The nose: the grille and lamps on the front face, and indicators wrapped round its corners. Each
    # is a panel a few millimetres off the face, those laid over others a little further out.
    face = hz + 0.003
    edge = hx - nr - 0.01
    lamp = c["lamps"]
    x0, x1 = lamp["x"]
    x1 = min(x1, edge)
    zl, zh = z(lamp["z"][0]), z(lamp["z"][1])
    g = c["grille"]
    gx = min(g["x"], x0 - 0.02)
    g0, g1 = z(g["z"][0]), z(g["z"][1])
    if g.get("frame"):
        body.decal((0.0, face, (g0 + g1) / 2), (2 * gx + 0.04, g1 - g0 + 0.04), FRONT, g["frame"])
    body.decal((0.0, face + 0.002, (g0 + g1) / 2), (2 * gx, g1 - g0), FRONT, DARK)
    for k in range(g.get("slats", 0)):
        h = g0 + (g1 - g0) * (k + 1) / (g["slats"] + 1)
        body.decal((0.0, face + 0.004, h), (2 * gx - 0.03, 0.014), FRONT, g.get("slat", STEEL))
    for sx in (-1, 1):
        if lamp.get("surround"):
            body.decal((sx * (x0 + x1) / 2, face, (zl + zh) / 2), (x1 - x0 + 0.03, zh - zl + 0.03), FRONT, lamp["surround"])
        if lamp["kind"] == "round2":
            r = (zh - zl) / 2
            body.decal((sx * (x1 - r), face + 0.002, (zl + zh) / 2), (2 * r,), FRONT, LAMP, sides=10)
            body.decal((sx * (x0 + r * 0.8), face + 0.002, (zl + zh) / 2), (1.6 * r,), FRONT, LAMP, sides=10)
        else:
            body.decal((sx * (x0 + x1) / 2, face + 0.002, (zl + zh) / 2), (x1 - x0, zh - zl), FRONT, LAMP)
            # A bar across each lamp, so that it reads as a lamp unit and not a light panel.
            for bar in lamp.get("bars", (0.5,)):
                body.decal((sx * (x0 + (x1 - x0) * bar), face + 0.004, (zl + zh) / 2), (0.014, zh - zl), FRONT, DARK)
    if c.get("indicators", "corner") == "corner":
        t = math.radians(45)
        for sx in (-1, 1):
            cx, cy = sx * (hx - nr + (nr + 0.003) * math.cos(t)), hz - nr + (nr + 0.003) * math.sin(t)
            body.decal((cx, cy, (zl + zh) / 2), (min(0.12, nr * 1.1), zh - zl), (sx * math.cos(t), math.sin(t), 0), AMBER)
    else:
        mirrored_decal(body, x1 - 0.05, face + 0.002, zl - 0.05, (0.1, 0.04), FRONT, AMBER)
    for fx, fh in lamp.get("fog", ()):
        mirrored_decal(body, fx, hz + bm["out"] + 0.003, z(fh), (0.12, 0.06), FRONT, LAMP)

    # The tail: lamps on the back face with their reversing and indicator segments, a black panel
    # between them on the cars that had one, the plate and the exhaust.
    tl = c["tail"]
    back_face = -hz - 0.003
    tx0, tx1 = tl["x"]
    tx1 = min(tx1, hx - tr - 0.01)
    tz0, tz1 = z(tl["z"][0]), z(tl["z"][1])
    if tl.get("panel"):
        body.decal((0.0, back_face, (tz0 + tz1) / 2), (2 * tx0, tz1 - tz0), BACK, tl["panel"])
    mirrored_decal(body, (tx0 + tx1) / 2, back_face, (tz0 + tz1) / 2, (tx1 - tx0, tz1 - tz0), BACK, RED)
    mirrored_decal(body, tx0 + 0.05, back_face - 0.002, tz0 + (tz1 - tz0) * 0.3, (0.08, (tz1 - tz0) * 0.4), BACK, WHITE)
    mirrored_decal(body, tx1 - 0.05, back_face - 0.002, tz0 + (tz1 - tz0) * 0.3, (0.08, (tz1 - tz0) * 0.4), BACK, AMBER)
    plate_z = tl.get("plate", (tl["z"][0] + tl["z"][1]) / 2)
    body.decal((0.0, back_face - 0.004, z(plate_z)), (0.5, 0.11), BACK, WHITE)
    body.cylinder((-hx * 0.55, -hz - bm["out"] + 0.02, z(bm["lo"]) - 0.02), 0.03, 0.12, DARK, axis="Y", segments=6)

    # Along the sides: door seams, handles, mirrors, side indicators, a rubbing strip and the fuel flap.
    for a in c["doors"]:
        hb, ht = bottom(a), along_line(c["top"], a)
        hs = max(ht - drop, hb + 0.6 * (ht - hb))
        mirrored_decal(body, width(a) + 0.002, y_of(a), z((hb + 0.06 + hs) / 2), (0.01, hs - hb - 0.08), (1, 0, 0), DARK)
    for a in c["doors"][1:]:
        ht = along_line(c["top"], a - 0.12)
        mirrored_decal(body, hx + 0.003, y_of(a - 0.12), z(ht - drop - 0.04), (0.11, 0.025), (1, 0, 0), c.get("handles", DARK))
    am = screen[0] + 0.12
    hm = along_line(c["top"], am)
    for sx in (-1, 1):
        obox(body, (sx * (hx + 0.03), y_of(am) - 0.03, z(hm) + 0.07), (0.12, 0.05, 0.08), DARK, rz=sx * 0.15)
    a = arches[0] + arch_r + 0.12
    mirrored_decal(body, width(a) + 0.003, y_of(a), z(along_line(c["top"], a) - drop - 0.06), (0.06, 0.025), (1, 0, 0), AMBER)
    strip = c.get("strip")
    if strip:
        spans = [(arches[0] + arch_r + 0.04, arches[1] - arch_r - 0.04), (max(0.2, nr), arches[0] - arch_r - 0.04), (arches[1] + arch_r + 0.04, length - max(0.2, tr))]
        for a0, a1 in spans:
            if a1 - a0 > 0.08:
                mirrored_decal(body, width((a0 + a1) / 2) + 0.004, y_of((a0 + a1) / 2), z(strip[0]), (a1 - a0, strip[2]), (1, 0, 0), strip[1])
    af = arches[1] - arch_r - 0.22
    body.decal((hx + 0.003, y_of(af), z(along_line(c["top"], af) - drop - 0.06)), (0.12, 0.1), (1, 0, 0), DARK)
    if details:
        details(body, dict(c, hx=hx, hz=hz, z=z, y_of=y_of, width=width, top=lambda a: along_line(c["top"], a), glass_at=lambda a: along_line(glass, a), roof=roof_h, screen=screen, back=back, inset=inset, face=face, back_face=back_face))

    # Whatever lies flat on a side over a front door (its seams, the handle, a strip, lettering) is cut
    # at the seams, and the piece between them goes with the door.
    flats, body.flat = body.flat, []
    yf, yr = y_of(d0), y_of(d1)
    for points, mat, normal in flats:
        if abs(normal[0]) < 0.7 or min(abs(p[0]) for p in points) < hx * 0.5:
            body.flat.append((points, mat, normal))
            continue
        for keep, part in ((lambda p: p[1] - yf, body), (lambda p: yf - p[1], doors[1 if normal[0] > 0 else -1]), (lambda p: yr - p[1], body)):
            poly = clip(points, keep) if part is body else clip(clip(points, keep), lambda p: p[1] - yr)
            if len(poly) >= 3 and max(p[1] for p in poly) - min(p[1] for p in poly) > 1e-5:
                part.flat.append((poly, mat, normal))

    # Each door's opening: jambs at the seams a little way into the body, so that an open door, or the
    # gap where one came off, shows the cabin behind it (the seats, the dash) over a floor at the
    # sill, and the ground does not show under them.
    lining = hx - 0.1
    profile = lambda a: [(x, h) for x, h, _ in side_of(a)]
    for sx in (-1, 1):
        for a, facing in ((d0, -1.0), (d1, 1.0)):
            points = profile(a)
            for (x0, h0), (x1, h1) in zip(points, points[1:]):
                body.flat.append(([(sx * x0, y_of(a), z(h0)), (sx * x1, y_of(a), z(h1)), (sx * lining, y_of(a), z(h1)), (sx * lining, y_of(a), z(h0))], DARK, (0.0, facing, 0.0)))
    rear = min(y for _, y, _ in to_blender(heads)) - 0.3
    body.box((0.0, (y_of(d0) + rear) / 2, door_foot + 0.02), (2 * lining, y_of(d0) - rear, 0.06), DARK)

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


# --------------------------------------------------------------------------------------------------
# The EPA traktor: an old boxy saloon cut down into a pickup. A wedge of a bonnet, the front doors
# and their windows, then a blank panel where the back doors were welded shut and a wall with a
# window and a slow-vehicle triangle behind the seats. The boot and the back of the cabin are one
# short open bed, with tall tail lamps either side of the plate and a tow ball under the bumper.
# Black bumpers and side strips, a sunroof, and big rims on thin tyres.
# --------------------------------------------------------------------------------------------------
def triangle(part, centre, side, depth, mat):
    """An upright triangle, point up, facing along Y: a slow-vehicle sign."""
    cx, cy, cz = centre
    h = side * math.sqrt(3) / 2
    outline = [(cx - side / 2, cz - h / 3), (cx + side / 2, cz - h / 3), (cx, cz + 2 * h / 3)]
    a = [part.bm.verts.new((x, cy - depth / 2, z)) for x, z in outline]
    b = [part.bm.verts.new((x, cy + depth / 2, z)) for x, z in outline]
    faces = [part.bm.faces.new(a), part.bm.faces.new(b[::-1])]
    for i in range(3):
        j = (i + 1) % 3
        faces.append(part.bm.faces.new((a[i], a[j], b[j], b[i])))
    for face in faces:
        face.material_index = mat


def build_epa():
    spec = SPECS["epa"]
    hx, hy = 0.86, spec["half"][2]
    axle, arch = spec["axle"], 0.4
    # Heights over the origin: the bottom of the sills, the top of the wheel arches, the waist line
    # under the windows, and the roof.
    sill, top, belt, roof = -0.22, 0.25, 0.45, 0.93
    # Along the car: the foot of the windscreen, its top, the door's back edge and the cab's back wall.
    screen, crown, door, wall = 0.9, 0.2, -0.22, -0.78
    body = Part("body")
    # The lower body, cut away round the wheels, with the wells dark between them.
    for y0, y1 in ((-hy, -axle - arch), (-axle + arch, axle - arch), (axle + arch, hy)):
        body.box((0.0, (y0 + y1) / 2, (sill + top) / 2), (2 * hx, y1 - y0, top - sill), PAINT)
    for y in (-axle, axle):
        body.box((0.0, y, (sill + top) / 2), (2 * (spec["track"] - spec["wheel_width"] / 2) - 0.02, 2 * arch, top - sill), DARK)
    # The bonnet and the wings, a wedge that rises to the windscreen.
    body.prism([(hy - 0.02, top), (hy - 0.02, 0.33), (screen, belt), (screen, top)], -hx, hx, PAINT)
    # The doors and the welded back doors up to the waist, a dash and two seats inside.
    mirrored(body, hx - 0.04, (screen + wall) / 2, (top + belt) / 2, (0.08, screen - wall, belt - top), PAINT)
    body.box((0.0, screen - 0.2, belt - 0.02), (2 * hx - 0.16, 0.4, 0.16), DARK)
    for side in (-1, 1):
        body.box((side * 0.38, -0.1, top + 0.06), (0.5, 0.5, 0.12), DARK)
        body.box((side * 0.38, -0.38, 0.55), (0.5, 0.1, 0.6), DARK, rx=-0.15)
    body.cylinder((-0.38, 0.45, 0.6), 0.18, 0.03, DARK, axis="Y", segments=12)
    # The windscreen and its posts, the roof with a sunroof, and the side glass up to the B post.
    rake = -math.atan2(roof - belt, screen - crown)
    rise = math.hypot(roof - belt, screen - crown)
    body.box((0.0, (screen + crown) / 2, (belt + roof) / 2), (1.5, rise, 0.03), GLASS, rx=rake)
    mirrored(body, 0.78, (screen + crown) / 2, (belt + roof) / 2, (0.07, rise, 0.07), PAINT, rx=rake)
    body.box((0.0, (crown + wall) / 2, roof - 0.03), (1.62, crown - wall + 0.04, 0.06), PAINT)
    body.box((0.0, -0.2, roof + 0.015), (0.7, 0.5, 0.03), DARK)
    for side in (-1, 1):
        x = side * 0.79
        body.prism([(screen - 0.04, belt), (crown + 0.02, roof - 0.06), (door, roof - 0.06), (door, belt)], x - 0.01, x + 0.01, GLASS)
        # The B post, and the blank panel where the back door's window was.
        body.box((x, door - 0.04, (belt + roof) / 2), (0.05, 0.08, roof - belt), PAINT)
        body.box((x, (door + wall) / 2, (belt + roof) / 2), (0.04, door - wall, roof - belt), PAINT)
    # The cab's back wall: paint round a big window, and the slow-vehicle triangle in it.
    body.box((0.0, wall, (top + belt) / 2), (2 * hx - 0.08, 0.06, belt - top), PAINT)
    mirrored(body, 0.71, wall, (belt + roof) / 2, (0.18, 0.06, roof - belt), PAINT)
    body.box((0.0, wall, belt + 0.03), (1.3, 0.06, 0.06), PAINT)
    body.box((0.0, wall, (belt + roof) / 2 + 0.01), (1.26, 0.03, roof - belt - 0.06), GLASS)
    triangle(body, (-0.2, wall - 0.03, belt + 0.22), 0.36, 0.01, AMBER)
    # The bed: its floor over the rear axle, its sides and the tailgate.
    body.box((0.0, (wall - hy) / 2, top + 0.015), (2 * hx - 0.08, wall + hy, 0.03), DARK)
    mirrored(body, hx - 0.04, (wall - hy) / 2, (top + belt) / 2, (0.08, wall + hy, belt - top), PAINT)
    body.box((0.0, -hy + 0.04, (top + belt) / 2), (2 * hx, 0.08, belt - top), PAINT)
    # A fuel flap on the left of the bed.
    body.box((-hx - 0.003, -1.0, 0.35), (0.01, 0.14, 0.12), WHITE)
    # The nose: a black bumper, a grille in a bright surround, square headlamps and amber indicators.
    body.box((0.0, hy + 0.03, -0.11), (2 * hx + 0.04, 0.16, 0.2), DARK)
    mirrored(body, hx - 0.02, hy - 0.2, -0.11, (0.08, 0.36, 0.2), DARK)
    body.box((0.0, hy - 0.01, 0.13), (0.66, 0.03, 0.24), STEEL)
    body.box((0.0, hy, 0.13), (0.58, 0.03, 0.18), DARK)
    mirrored(body, 0.55, hy - 0.01, 0.14, (0.36, 0.04, 0.16), LAMP)
    mirrored(body, 0.8, hy - 0.02, 0.14, (0.1, 0.04, 0.16), AMBER)
    # Black strips down the sides, door seams, handles and mirrors.
    mirrored(body, hx + 0.01, 0.0, 0.05, (0.02, 2 * (axle - arch) - 0.1, 0.06), DARK)
    for y in (screen, door):
        mirrored(body, hx + 0.002, y, (sill + belt) / 2 + 0.05, (0.01, 0.012, belt - sill - 0.1), DARK)
    mirrored(body, hx + 0.01, door + 0.15, belt - 0.07, (0.02, 0.12, 0.03), DARK)
    mirrored(body, hx + 0.08, screen - 0.1, belt + 0.1, (0.12, 0.07, 0.1), DARK)
    # The tail: tall lamps either side of the plate, the bumper, a tow ball and the exhaust.
    for side in (-1, 1):
        x = side * (hx - 0.11)
        body.box((x, -hy - 0.005, 0.2), (0.2, 0.03, 0.4), RED)
        body.box((x, -hy - 0.015, 0.2), (0.2, 0.02, 0.08), WHITE)
        body.box((x, -hy - 0.015, 0.12), (0.2, 0.02, 0.05), AMBER)
    body.box((0.0, -hy - 0.005, 0.22), (0.6, 0.02, 0.17), DARK)
    body.box((0.0, -hy - 0.015, 0.22), (0.52, 0.02, 0.11), WHITE)
    body.box((0.0, -hy - 0.04, -0.11), (2 * hx + 0.04, 0.16, 0.22), DARK)
    mirrored(body, hx - 0.02, -hy + 0.2, -0.11, (0.08, 0.36, 0.22), DARK)
    body.box((0.0, -hy - 0.16, -0.24), (0.06, 0.26, 0.05), DARK)
    body.box((0.0, -hy - 0.27, -0.19), (0.03, 0.03, 0.08), STEEL)
    body.cylinder((0.0, -hy - 0.27, -0.13), 0.045, 0.06, STEEL, segments=8)
    body.cylinder((-0.45, -hy - 0.02, -0.24), 0.035, 0.2, DARK, axis="Y", segments=6)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=0, rim_ratio=0.7)
    # EPA_SEAT_HEADS, in this frame: x, -z, y.
    add_seats([(-0.38, -0.1, 0.75), (0.38, -0.1, 0.75)])
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The period cars, 1980 to 1995 (VEHICLES-XXIII.md): one body builder, `build_saloon`, drawn from a
# side profile and a glasshouse, and each car's own shape and details on top of it.
# --------------------------------------------------------------------------------------------------
# Each period car's <KIND>_SEAT_HEADS, in the game's hull frame (x, y, z). Keep in step with crates/shared/src/vehicles/.
HEADS = {
    "saab_900": [(-0.38, 0.71, 0.0), (0.38, 0.71, 0.0), (-0.4, 0.69, 0.88), (0.4, 0.69, 0.88)],
    "saab_9000": [(-0.38, 0.7, 0.0), (0.38, 0.7, 0.0), (-0.4, 0.68, 0.88), (0.4, 0.68, 0.88), (0.0, 0.68, 0.88)],
    "volvo_240": [(-0.38, 0.67, 0.0), (0.38, 0.67, 0.0), (-0.4, 0.65, 0.88), (0.4, 0.65, 0.88), (0.0, 0.65, 0.88)],
    "volvo_740": [(-0.38, 0.69, 0.0), (0.38, 0.69, 0.0), (-0.4, 0.67, 0.88), (0.4, 0.67, 0.88), (0.0, 0.67, 0.88)],
    "gti": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75)],
    "transporter": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
    "kadett": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75), (0.0, 0.71, 0.75)],
    "omega": [(-0.38, 0.7, 0.0), (0.38, 0.7, 0.0), (-0.4, 0.68, 0.88), (0.4, 0.68, 0.88), (0.0, 0.68, 0.88)],
    "sierra": [(-0.38, 0.71, 0.0), (0.38, 0.71, 0.0), (-0.4, 0.69, 0.88), (0.4, 0.69, 0.88), (0.0, 0.69, 0.88)],
    "escort": [(-0.38, 0.73, -0.05), (0.38, 0.73, -0.05), (-0.4, 0.71, 0.75), (0.4, 0.71, 0.75), (0.0, 0.71, 0.75)],
    "sports": [(-0.37, 0.72, 0.1), (0.37, 0.72, 0.1), (-0.38, 0.66, 0.85), (0.38, 0.66, 0.85)],
    "police_car": [(-0.38, 0.67, 0.0), (0.38, 0.67, 0.0), (-0.4, 0.65, 0.88), (0.4, 0.65, 0.88), (0.0, 0.65, 0.88)],
    "ambulance": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
    "police_van": [(-0.45, 0.87, -1.25), (0.45, 0.87, -1.25), (-0.4, 0.87, 0.05), (0.4, 0.87, 0.05), (-0.5, 0.87, 1.15), (0.0, 0.87, 1.15), (0.5, 0.87, 1.15)],
}


def hexa(part, corners, mat):
    """A six-faced solid from its eight corners: four round the bottom, then the four above them in the same order."""
    verts = [part.bm.verts.new(c) for c in corners]
    bottom, top = verts[:4], verts[4:]
    faces = [part.bm.faces.new(bottom[::-1]), part.bm.faces.new(top)]
    for i in range(4):
        j = (i + 1) % 4
        faces.append(part.bm.faces.new((bottom[i], bottom[j], top[j], top[i])))
    for face in faces:
        face.material_index = mat


def beam(part, p0, p1, thick, mat, broad=None):
    """A bar from point p0 to point p1, `thick` across and `broad` (as thick, unless given) the other way
    round: a window post, a roll bar. For a bar in a side of the car, `thick` is across the car."""
    a, b = Vector(p0), Vector(p1)
    along = (b - a).normalized()
    u = along.cross(Vector((0, 0, 1)) if abs(along.z) < 0.9 else Vector((1, 0, 0))).normalized() * thick / 2
    v = along.cross(u).normalized() * (thick if broad is None else broad) / 2
    ring = [u + v, -u + v, -u - v, u - v]
    hexa(part, [tuple(a + r) for r in ring] + [tuple(b + r) for r in ring], mat)


def clip(poly, keep):
    """The convex outline `poly` (of (y, z) or (x, y, z) points) cut down to where the linear `keep` is
    not negative."""
    out = []
    for i, p in enumerate(poly):
        q = poly[(i + 1) % len(poly)]
        fp, fq = keep(p), keep(q)
        if fp >= 0:
            out.append(p)
        if (fp >= 0) != (fq >= 0):
            t = fp / (fp - fq)
            out.append(tuple(u + t * (v - u) for u, v in zip(p, q)))
    return out


# How far an arch's edge falls for each metre it is from the axle.
ARCH_FALL = 0.45


def round_wheels(body, spec, profile, x0, x1, mat, arch_top):
    """Extrudes the side `profile` from x0 to x1, leaving room over each wheel up to `arch_top`."""
    axle, arch = spec["axle"], spec["wheel_radius"] + 0.08
    # Each arch is cut highest over the axle and falls away to either side, in two halves.
    cuts = [(-9.0, -axle - arch, None), (-axle - arch, -axle, -axle), (-axle, -axle + arch, -axle), (-axle + arch, axle - arch, None), (axle - arch, axle, axle), (axle, axle + arch, axle), (axle + arch, 9.0, None)]
    for y0, y1, over in cuts:
        poly = clip(clip(profile, lambda p: p[0] - y0), lambda p: y1 - p[0])
        if over is not None:
            fall = ARCH_FALL if y0 >= over else -ARCH_FALL
            poly = clip(poly, lambda p: p[1] - arch_top + fall * (p[0] - over))
        if len(poly) >= 3:
            body.prism(poly, x0, x1, mat)


def wheel_wells(body, spec, sill, arch_top):
    """The dark wells the wheels turn in, inside the body between them."""
    inner = spec["track"] - spec["wheel_width"] / 2 - 0.02
    for y in (-spec["axle"], spec["axle"]):
        body.box((0.0, y, (sill + arch_top) / 2), (2 * inner, 2 * (spec["wheel_radius"] + 0.08), arch_top - sill), DARK)


def seats_for(body, heads, wheel=True):
    """A cushion and a back under each rider's head (Blender frame), and the driver's steering wheel."""
    for x, y, z in heads:
        body.box((x, y + 0.05, z - 0.7), (0.46, 0.48, 0.12), DARK)
        body.box((x, y - 0.22, z - 0.38), (0.46, 0.1, 0.56), DARK, rx=-0.12)
    if wheel:
        x, y, z = heads[0]
        body.cylinder((x, y + 0.45, z - 0.3), 0.18, 0.03, DARK, axis="Y", segments=12)


def cabin(body, heads):
    """The seats under the riders' heads (Blender frame): one each in the front row, a bench across
    each row behind it, and the driver's steering wheel."""
    rows = {}
    for x, y, z in heads:
        rows.setdefault(round(y, 2), []).append((x, z))
    for k, (y, row) in enumerate(sorted(rows.items(), reverse=True)):
        h = min(z for _, z in row)
        if k == 0:
            for x, _ in row:
                body.box((x, y + 0.05, h - 0.7), (0.46, 0.48, 0.12), DARK)
                body.box((x, y - 0.22, h - 0.38), (0.46, 0.1, 0.56), DARK, rx=-0.12)
        else:
            xs = [x for x, _ in row]
            span = max(xs) - min(xs) + 0.5
            body.box(((max(xs) + min(xs)) / 2, y + 0.05, h - 0.7), (span, 0.48, 0.12), DARK)
            body.box(((max(xs) + min(xs)) / 2, y - 0.22, h - 0.38), (span, 0.1, 0.56), DARK, rx=-0.12)
    x, y, z = heads[0]
    body.cylinder((x, y + 0.45, z - 0.3), 0.18, 0.03, DARK, axis="Y", segments=8)


def mirrored_decal(body, x, y, z, size, normal, mat):
    """The same panel on both sides of the car, at +/-x, each facing out (or both along `normal` when it faces along the car)."""
    for side in (-1, 1):
        n = (side * normal[0], normal[1], normal[2])
        body.decal((side * x, y, z), size, n, mat)


def to_blender(heads):
    """SEAT_HEADS in the game's hull frame, (x, y, z), as Blender points: (x, -z, y)."""
    return [(x, -z, y) for x, y, z in heads]


def obox(part, centre, size, mat, rz=0.0, rx=0.0):
    """A box turned `rz` about the vertical, then `rx` about its own across-axis."""
    m = Matrix.Translation(centre) @ Matrix.Rotation(rz, 4, "Z") @ Matrix.Rotation(rx, 4, "X") @ Matrix.Diagonal((*size, 1.0))
    part._tag(bmesh.ops.create_cube(part.bm, size=1.0, matrix=m)["verts"], mat)


def shell(part, rings, mat_of, cap=None):
    """A skin through closed rings of points, each ring with as many points: the face between ring `s`
    and the next along edge `k` of the ring (point `k` to the next) is `mat_of(s, k)`. With `cap`, the
    first and last rings are closed in that material. A face of material None is one nobody sees,
    inside the body, and is left out."""
    verts = [[part.bm.verts.new(p) for p in ring] for ring in rings]
    n = len(verts[0])
    for s, (a, b) in enumerate(zip(verts, verts[1:])):
        for k in range(n):
            mat = mat_of(s, k)
            if mat is None:
                continue
            j = (k + 1) % n
            part.bm.faces.new((a[k], a[j], b[j], b[k])).material_index = mat
    if cap is not None:
        part.bm.faces.new(verts[0]).material_index = cap
        part.bm.faces.new(verts[-1][::-1]).material_index = cap


def along_line(points, a):
    """The height of the line through `points`, (along, height) from the front back, at `along`."""
    if a <= points[0][0]:
        return points[0][1]
    for (a0, h0), (a1, h1) in zip(points, points[1:]):
        if a <= a1:
            return h0 + (h1 - h0) * (a - a0) / max(a1 - a0, 1e-6)
    return points[-1][1]


def wheel_face(part, x, out, radii, segments, pattern):
    """The outer face of a wheel, `x` along its axle facing `out`: rings at `radii` round its centre,
    each sector of ring `r` (0 the innermost) in material `pattern(r, k)`. The spokes, holes and slots
    are colours on it, not solids, so a wheel face is a few dozen triangles however many spokes it has."""
    ring = lambda radius: [(x, radius * math.cos(2 * math.pi * k / segments), radius * math.sin(2 * math.pi * k / segments)) for k in range(segments)]
    rings = [ring(r) for r in radii]
    hub = (x + out * 0.006, 0.0, 0.0)
    normal = (float(out), 0.0, 0.0)
    for k in range(segments):
        j = (k + 1) % segments
        part.flat.append(([hub, rings[0][k], rings[0][j]], pattern(0, k), normal))
        for r in range(1, len(radii)):
            part.flat.append(([rings[r - 1][k], rings[r][k], rings[r][j], rings[r - 1][j]], pattern(r, k), normal))


def styled_wheels(spec, style, rim_ratio=0.62):
    """Four wheels on steering pivots, each a tyre with its face in `style`: 'steel' (a black steel
    wheel under a hubcap with slots), 'dial' (a flat disc with round holes) or 'spokes3', 'spokes5' and
    'spokes6' (an alloy with that many spokes)."""
    radius, width = spec["wheel_radius"], spec["wheel_width"]
    z = radius - spec["ride"]
    rim = radius * rim_ratio
    built = []
    for fore, y in (("F", spec["axle"]), ("R", -spec["axle"])):
        for side, x in (("L", -spec["track"]), ("R", spec["track"])):
            tag, out = fore + side, (-1 if side == "L" else 1)
            pivot = empty(f"pivot_{tag}", (x, y, z))
            wheel = Part(f"wheel_{tag}", parent=pivot)
            wheel.cylinder((0, 0, 0), radius, width, DARK, axis="X", segments=12)
            face = out * (width / 2 + 0.004)
            if style == "steel":
                wheel_face(wheel, face, out, (rim * 0.78, rim), 12, lambda r, k: (DARK if k % 3 == 0 else STEEL) if r == 0 else DARK)
            elif style == "dial":
                wheel_face(wheel, face, out, (rim * 0.3, rim * 0.82, rim), 12, lambda r, k: DARK if r == 1 and k % 2 == 0 else STEEL)
            else:
                spokes = int(style[len("spokes"):])
                every = 12 // spokes
                wheel_face(wheel, face, out, (rim * 0.28, rim * 0.86, rim), 12, lambda r, k: STEEL if r != 1 or k % every == 0 else DARK)
            built.append(wheel.build())
    return built


def wrap(part, spec, end, lo, hi, out, deep, mat, nr, back=0.3):
    """A bumper round the front (`end` 1) or the back (-1) of the car, from `lo` to `hi` (Blender z),
    standing `out` proud of the body and `deep` into it, round its corners (radius `nr`) and back along
    its sides by `back`."""
    hx, hz = spec["half"][0], spec["half"][2]
    right = [(hx, hz - nr - back, 1.0, 0.0)]
    for deg in (0, 45, 90):
        t = math.radians(deg)
        right.append((hx - nr + nr * math.cos(t), hz - nr + nr * math.sin(t), math.cos(t), math.sin(t)))
    path = right + [(-x, y, -nx, ny) for x, y, nx, ny in reversed(right)]
    rings = []
    for x, y, nx, ny in path:
        ring = [(x - nx * deep, y - ny * deep, lo), (x + nx * out, y + ny * out, lo), (x + nx * out, y + ny * out, hi), (x - nx * deep, y - ny * deep, hi)]
        rings.append([(px, end * py, pz) for px, py, pz in ring])
    shell(part, rings, lambda s, k: None if k == 3 else mat, cap=mat)


def build_saloon(name, c, heads, details=None):
    """A closed car of the 1980s, from `c`: heights above the ground and lengths back from the front
    face, metres. `top` is the line along the top of the lower body (bonnet, waist, boot or tail) and
    `glass` the line along the top of the glasshouse (windscreen, roof, back glass), both as (along,
    height) from front to back. The body is lofted through cross-sections: rounded at the corners in
    plan (`nose_round`, `tail_round`), its sides tucked in at the sill and leaning in from a shoulder
    crease, and cut round the wheels in arches. `heads` is the car's SEAT_HEADS, and `details` draws
    whatever is its own on top, given the measurements worked out here."""
    spec = SPECS[name]
    hx, hz = spec["half"][0], spec["half"][2]
    length, ground = 2 * hz, -spec["ride"]
    radius, axle = spec["wheel_radius"], spec["axle"]
    z = lambda h: h + ground
    y_of = lambda a: hz - a
    nr, tr = c["nose_round"], c["tail_round"]
    sill = c["sill"]
    drop, inset = c.get("shoulder", (0.09, 0.05))
    tuck = c.get("tuck", 0.05)
    upper = c.get("upper", PAINT)
    clad = c.get("clad")

    def width(a):
        """Half the body's width `a` back from the front face, rounded at both ends."""
        if a < nr:
            d = (nr - a) / nr
            return hx - nr + nr * math.sqrt(max(0.0, 1 - d * d))
        if a > length - tr:
            d = (a - (length - tr)) / tr
            return hx - tr + tr * math.sqrt(max(0.0, 1 - d * d))
        return hx

    arch_r = radius + 0.06
    centre_h = radius
    arches = [hz - axle, hz + axle]

    def bottom(a):
        """The underside of the body's side at `a`: the sill, rising under the bumpers and over the wheels."""
        h = sill
        front, back = c.get("chin", (sill, 0.3)), c.get("tail_lift", (sill, 0.3))
        if a < front[1]:
            h = max(h, front[0] + (sill - front[0]) * a / front[1])
        if a > length - back[1]:
            h = max(h, back[0] + (sill - back[0]) * (length - a) / back[1])
        for at in arches:
            if abs(a - at) <= arch_r + 1e-6:
                h = max(h, centre_h + math.sqrt(max(0.0, arch_r * arch_r - (a - at) ** 2)))
        return h

    # Where the cross-sections are: round the corners, at every bend of the top line, and round each arch.
    stations = {0.0, length}
    t = math.radians(45)
    stations |= {nr * (1 - math.cos(t)), length - tr * (1 - math.cos(t))}
    stations |= {nr, length - tr, c.get("chin", (0, 0.3))[1], length - c.get("tail_lift", (0, 0.3))[1]}
    stations |= {a for a, _ in c["top"] if 0 < a < length}
    for at in arches:
        stations |= {at - arch_r - 0.004, at + arch_r + 0.004}
        stations |= {at - arch_r * math.cos(math.radians(deg)) for deg in (0, 45, 90, 135, 180)}
    # The front door's seams are stations of their own, so its faces end exactly on them; a station
    # close to a seam gives way to it.
    d0, d1 = c["doors"][0], c["doors"][1]
    seams = lambda points: sorted({a for a in points if min(abs(a - d0), abs(a - d1)) > 0.003} | {d0, d1})
    stations = seams(stations)
    merged = []
    for a in stations:
        if not merged or a - merged[-1] > 0.002:
            merged.append(a)
    in_door = lambda a0, a1: a0 >= d0 - 1e-6 and a1 <= d1 + 1e-6

    def side_of(a):
        """The right half of the cross-section `a` back from the front face, bottom to top, as (out,
        height, the material of the face up to the next point)."""
        w, hb, ht = width(a), bottom(a), along_line(c["top"], a)
        hs = max(ht - drop, hb + 0.6 * (ht - hb))
        hl = hb + min(0.07, (hs - hb) * 0.3)
        lower = clad[1] if clad else PAINT
        half = [(w - tuck, hb, lower), (w, hl, lower)]
        if clad:
            half.append((w, min(max(clad[0], hl + 0.01), hs - 0.01), PAINT))
        return half + [(w, hs, PAINT), (w - inset, ht, PAINT)]

    rings, mats = [], []
    for a in merged:
        hb, ht = bottom(a), along_line(c["top"], a)
        half = side_of(a)
        ring = [(0.0, hb)] + [(x, h) for x, h, _ in half] + [(0.0, ht)] + [(-x, h) for x, h, _ in half[::-1]]
        mats = [DARK] + [m for _, _, m in half[:-1]] + [PAINT, PAINT] + [m for _, _, m in half[-2::-1]] + [DARK]
        rings.append([(x, y_of(a), z(h)) for x, h in ring])
    body = Part("body")
    # The top of the lower body under the glasshouse is not seen.
    glass = c["glass"]
    under = lambda s: merged[s] >= glass[0][0] + 0.01 and merged[s + 1] <= glass[-1][0] - 0.01
    top_edges = {len(half), len(half) + 1}
    # The front doors: each side's faces between the seams, from the sill to the shoulder, cut out of
    # the body into a door of its own. The left side is the driver's (-X).
    H = len(half)
    sides = {1: set(range(1, H)), -1: set(range(H + 2, 2 * H + 1))}
    doors = {-1: Door("door_FL", -1), 1: Door("door_FR", 1)}
    door_face = lambda s, k: in_door(merged[s], merged[s + 1]) and next((sx for sx, ks in sides.items() if k in ks), None)
    shell(body, rings, lambda s, k: None if (k in top_edges and under(s)) or door_face(s, k) else mats[k], cap=PAINT)
    for sx, door in doors.items():
        shell(door, rings, lambda s, k: mats[k] if door_face(s, k) == sx else None)
    # The dark wells the wheels turn in.
    inner = spec["track"] - spec["wheel_width"] / 2 - 0.02
    for at in arches:
        body.box((0.0, y_of(at), z((sill + centre_h + arch_r) / 2)), (2 * inner, 2 * arch_r, centre_h + arch_r - sill), DARK)

    # The glasshouse, lofted along the glass line and leaning in towards the top (`tumble`). Each
    # cross-section is a side up to the top of the side glass, a band round the corner as broad as an A
    # post (`a_post`), and a top across. The band is painted all along, so it is the A posts up the
    # windscreen's edges, the roof's edges and the rear posts down the back glass's, all flush with the
    # glass. The top is the windscreen, the roof and the back glass. The windscreen stops where the line
    # flattens into the roof, so the curve over the top of it is painted.
    roof_h = max(h for _, h in glass)
    tumble = c.get("tumble", 0.12)
    pw = c.get("a_post", 0.07)
    gstations = seams({a for a, _ in glass})
    grings = []
    for a in gstations:
        wb = width(a) - inset - 0.005
        hb = along_line(c["top"], a)
        hg = max(along_line(glass, a), hb + 0.02)
        f = (hg - hb) / max(roof_h - hb, 0.01)
        wt = wb - tumble * f
        e = min(0.05, (hg - hb) / 2)
        half = [(wb, hb - 0.01), (wt, hg - e), (wt - pw, hg)]
        grings.append([(x, y_of(a), z(h)) for x, h in [(0.0, hb - 0.01)] + half + [(0.0, hg)] + [(-x, h) for x, h in half[::-1]]])
    screen = [a for a, h in glass if a <= next(a for a, h in glass if h > roof_h - 0.025)]
    back = [a for a, h in glass if a >= [a for a, h in glass if h > roof_h - 0.025][-1]]
    screen_top = next((a0 for (a0, h0), (a1, h1) in zip(glass, glass[1:]) if (h1 - h0) < 0.6 * (a1 - a0)), screen[-1])
    roofed = lambda s: gstations[s] >= screen_top - 1e-6 and gstations[s + 1] <= back[0] + 1e-6
    shell(body, grings, lambda s, k: None if k in (0, 1, 6, 7) else upper if k in (2, 5) or roofed(s) else GLASS, cap=GLASS)

    # The sides: glass, with the other posts painted into it as seen from the side. A B post is a band
    # `b_post` broad round each of `posts`; the rear post is everything behind a line `c_post` ahead of
    # the back glass and running down with it.
    def ahead(a0, h0, a1, h1):
        """How far a point is ahead of the line through (a0, h0) and (a1, h1) seen from the side, metres."""
        return lambda p: a0 + (a1 - a0) * (p[2] - ground - h0) / (h1 - h0) - (hz - p[1])
    behind = lambda f: (lambda p: -f(p))
    bw = c.get("b_post", 0.08)
    regions = [([ahead(a + bw / 2, 0.0, a + bw / 2, 1.0), behind(ahead(a - bw / 2, 0.0, a - bw / 2, 1.0))], c.get("b_mat", upper)) for a in c.get("posts", ())]
    cp = c.get("c_post", 0.1)
    regions.append(([behind(ahead(back[0] - cp, roof_h, back[-1] - cp, along_line(c["top"], back[-1])))], upper))

    def area(poly):
        n = Vector()
        for p, q in zip(poly, poly[1:] + poly[:1]):
            n += Vector(p).cross(Vector(q))
        return n.length / 2

    for s, (ra, rb) in enumerate(zip(grings, grings[1:])):
        for i, j, out in ((1, 2, 1.0), (6, 7, -1.0)):
            pieces = [([ra[i], ra[j], rb[j], rb[i]], GLASS)]
            for keeps, mat in regions:
                split = []
                for poly, m in pieces:
                    if m != GLASS:
                        split.append((poly, m))
                        continue
                    for keep in keeps:
                        split.append((clip(poly, behind(keep)), GLASS))
                        poly = clip(poly, keep)
                        if len(poly) < 3:
                            break
                    split.append((poly, mat))
                pieces = [(poly, m) for poly, m in split if len(poly) >= 3 and area(poly) > 1e-8]
            # They meet their neighbours only at points along an edge, so they are faced out by hand.
            if in_door(gstations[s], gstations[s + 1]):
                doors[int(out)].skin += pieces
            else:
                body.flat += [(poly, m, (out, 0.0, 0.0)) for poly, m in pieces]
    # The doors' tops are the tops of their side glass; the posts' band over them stays on the body.
    door_top = max(r[2][2] for a, r in zip(gstations, grings) if d0 - 1e-6 <= a <= d1 + 1e-6)
    door_foot = z(min(bottom(a) for a in merged if d0 - 1e-6 <= a <= d1 + 1e-6))
    for door in doors.values():
        door.close()

    for sx in (1, -1):
        # The trim along the foot of the side windows, the piece on the front door with it.
        a0, a1 = screen[0] + 0.05, back[-1] - 0.05
        for p0, p1, part in ((a0, min(a1, d0), body), (max(a0, d0), min(a1, d1), doors[sx]), (max(a0, d1), a1, body)):
            if p1 - p0 > 0.01:
                hb0, hb1 = along_line(c["top"], p0), along_line(c["top"], p1)
                beam(part, (sx * (width(p0) - inset + 0.004), y_of(p0), z(hb0) + 0.012), (sx * (width(p1) - inset + 0.004), y_of(p1), z(hb1) + 0.012), 0.02, c.get("trim", DARK), 0.025)

    # Inside: a dash under the windscreen, the seats and the wheel.
    hd = along_line(c["top"], screen[0])
    body.box((0.0, y_of(screen[0]) - 0.22, z(hd) - 0.06), (2 * hx - 0.24, 0.36, 0.14), DARK)
    cabin(body, to_blender(heads))
    # Wipers lying at the foot of the windscreen.
    a = screen[0] + 0.07
    hw = along_line(glass, a) + 0.012
    for x0, x1 in ((-0.62, -0.08), (-0.02, 0.5)):
        beam(body, (x0, y_of(a + 0.02), z(hw + 0.02)), (x1, y_of(a), z(hw)), 0.015, DARK)

    # Bumpers, wrapped round the corners, with the plates on them.
    bm = c["bumpers"]
    wrap(body, spec, 1, z(bm["lo"]), z(bm["hi"]), bm["out"], 0.1, bm["mat"], nr, bm.get("back", 0.3))
    wrap(body, spec, -1, z(bm["lo"]), z(bm["hi"]), bm["out"], 0.1, bm["mat"], tr, bm.get("back", 0.3))
    FRONT, BACK = (0, 1, 0), (0, -1, 0)
    body.decal((0.0, hz + bm["out"] + 0.003, z((bm["lo"] + bm["hi"]) / 2)), (0.5, min(0.11, bm["hi"] - bm["lo"] - 0.03)), FRONT, WHITE)

    # The nose: the grille and lamps on the front face, and indicators wrapped round its corners. Each
    # is a panel a few millimetres off the face, those laid over others a little further out.
    face = hz + 0.003
    edge = hx - nr - 0.01
    lamp = c["lamps"]
    x0, x1 = lamp["x"]
    x1 = min(x1, edge)
    zl, zh = z(lamp["z"][0]), z(lamp["z"][1])
    g = c["grille"]
    gx = min(g["x"], x0 - 0.02)
    g0, g1 = z(g["z"][0]), z(g["z"][1])
    if g.get("frame"):
        body.decal((0.0, face, (g0 + g1) / 2), (2 * gx + 0.04, g1 - g0 + 0.04), FRONT, g["frame"])
    body.decal((0.0, face + 0.002, (g0 + g1) / 2), (2 * gx, g1 - g0), FRONT, DARK)
    for k in range(g.get("slats", 0)):
        h = g0 + (g1 - g0) * (k + 1) / (g["slats"] + 1)
        body.decal((0.0, face + 0.004, h), (2 * gx - 0.03, 0.014), FRONT, g.get("slat", STEEL))
    for sx in (-1, 1):
        if lamp.get("surround"):
            body.decal((sx * (x0 + x1) / 2, face, (zl + zh) / 2), (x1 - x0 + 0.03, zh - zl + 0.03), FRONT, lamp["surround"])
        if lamp["kind"] == "round2":
            r = (zh - zl) / 2
            body.decal((sx * (x1 - r), face + 0.002, (zl + zh) / 2), (2 * r,), FRONT, LAMP, sides=10)
            body.decal((sx * (x0 + r * 0.8), face + 0.002, (zl + zh) / 2), (1.6 * r,), FRONT, LAMP, sides=10)
        else:
            body.decal((sx * (x0 + x1) / 2, face + 0.002, (zl + zh) / 2), (x1 - x0, zh - zl), FRONT, LAMP)
            # A bar across each lamp, so that it reads as a lamp unit and not a light panel.
            for bar in lamp.get("bars", (0.5,)):
                body.decal((sx * (x0 + (x1 - x0) * bar), face + 0.004, (zl + zh) / 2), (0.014, zh - zl), FRONT, DARK)
    if c.get("indicators", "corner") == "corner":
        t = math.radians(45)
        for sx in (-1, 1):
            cx, cy = sx * (hx - nr + (nr + 0.003) * math.cos(t)), hz - nr + (nr + 0.003) * math.sin(t)
            body.decal((cx, cy, (zl + zh) / 2), (min(0.12, nr * 1.1), zh - zl), (sx * math.cos(t), math.sin(t), 0), AMBER)
    else:
        mirrored_decal(body, x1 - 0.05, face + 0.002, zl - 0.05, (0.1, 0.04), FRONT, AMBER)
    for fx, fh in lamp.get("fog", ()):
        mirrored_decal(body, fx, hz + bm["out"] + 0.003, z(fh), (0.12, 0.06), FRONT, LAMP)

    # The tail: lamps on the back face with their reversing and indicator segments, a black panel
    # between them on the cars that had one, the plate and the exhaust.
    tl = c["tail"]
    back_face = -hz - 0.003
    tx0, tx1 = tl["x"]
    tx1 = min(tx1, hx - tr - 0.01)
    tz0, tz1 = z(tl["z"][0]), z(tl["z"][1])
    if tl.get("panel"):
        body.decal((0.0, back_face, (tz0 + tz1) / 2), (2 * tx0, tz1 - tz0), BACK, tl["panel"])
    mirrored_decal(body, (tx0 + tx1) / 2, back_face, (tz0 + tz1) / 2, (tx1 - tx0, tz1 - tz0), BACK, RED)
    mirrored_decal(body, tx0 + 0.05, back_face - 0.002, tz0 + (tz1 - tz0) * 0.3, (0.08, (tz1 - tz0) * 0.4), BACK, WHITE)
    mirrored_decal(body, tx1 - 0.05, back_face - 0.002, tz0 + (tz1 - tz0) * 0.3, (0.08, (tz1 - tz0) * 0.4), BACK, AMBER)
    plate_z = tl.get("plate", (tl["z"][0] + tl["z"][1]) / 2)
    body.decal((0.0, back_face - 0.004, z(plate_z)), (0.5, 0.11), BACK, WHITE)
    body.cylinder((-hx * 0.55, -hz - bm["out"] + 0.02, z(bm["lo"]) - 0.02), 0.03, 0.12, DARK, axis="Y", segments=6)

    # Along the sides: door seams, handles, mirrors, side indicators, a rubbing strip and the fuel flap.
    for a in c["doors"]:
        hb, ht = bottom(a), along_line(c["top"], a)
        hs = max(ht - drop, hb + 0.6 * (ht - hb))
        mirrored_decal(body, width(a) + 0.002, y_of(a), z((hb + 0.06 + hs) / 2), (0.01, hs - hb - 0.08), (1, 0, 0), DARK)
    for a in c["doors"][1:]:
        ht = along_line(c["top"], a - 0.12)
        mirrored_decal(body, hx + 0.003, y_of(a - 0.12), z(ht - drop - 0.04), (0.11, 0.025), (1, 0, 0), c.get("handles", DARK))
    am = screen[0] + 0.12
    hm = along_line(c["top"], am)
    for sx in (-1, 1):
        obox(body, (sx * (hx + 0.03), y_of(am) - 0.03, z(hm) + 0.07), (0.12, 0.05, 0.08), DARK, rz=sx * 0.15)
    a = arches[0] + arch_r + 0.12
    mirrored_decal(body, width(a) + 0.003, y_of(a), z(along_line(c["top"], a) - drop - 0.06), (0.06, 0.025), (1, 0, 0), AMBER)
    strip = c.get("strip")
    if strip:
        spans = [(arches[0] + arch_r + 0.04, arches[1] - arch_r - 0.04), (max(0.2, nr), arches[0] - arch_r - 0.04), (arches[1] + arch_r + 0.04, length - max(0.2, tr))]
        for a0, a1 in spans:
            if a1 - a0 > 0.08:
                mirrored_decal(body, width((a0 + a1) / 2) + 0.004, y_of((a0 + a1) / 2), z(strip[0]), (a1 - a0, strip[2]), (1, 0, 0), strip[1])
    af = arches[1] - arch_r - 0.22
    body.decal((hx + 0.003, y_of(af), z(along_line(c["top"], af) - drop - 0.06)), (0.12, 0.1), (1, 0, 0), DARK)
    if details:
        details(body, dict(c, hx=hx, hz=hz, z=z, y_of=y_of, width=width, top=lambda a: along_line(c["top"], a), glass_at=lambda a: along_line(glass, a), roof=roof_h, screen=screen, back=back, inset=inset, face=face, back_face=back_face))

    # Whatever lies flat on a side over a front door (its seams, the handle, a strip, lettering) is cut
    # at the seams, and the piece between them goes with the door.
    flats, body.flat = body.flat, []
    yf, yr = y_of(d0), y_of(d1)
    for points, mat, normal in flats:
        if abs(normal[0]) < 0.7 or min(abs(p[0]) for p in points) < hx * 0.5:
            body.flat.append((points, mat, normal))
            continue
        for keep, part in ((lambda p: p[1] - yf, body), (lambda p: yf - p[1], doors[1 if normal[0] > 0 else -1]), (lambda p: yr - p[1], body)):
            poly = clip(points, keep) if part is body else clip(clip(points, keep), lambda p: p[1] - yr)
            if len(poly) >= 3 and max(p[1] for p in poly) - min(p[1] for p in poly) > 1e-5:
                part.flat.append((poly, mat, normal))

    # Behind each door, a dark lining a little inside the body's side from the sill to the waist, the
    # jambs at the seams and a sill under the window, so an open door shows the cabin and not the
    # body's skin from inside.
    lining = hx - 0.1
    inside = [a for a in merged if d0 - 1e-6 <= a <= d1 + 1e-6]
    profile = lambda a: [(x, h) for x, h, _ in side_of(a)]
    for sx in (-1, 1):
        out = (sx, 0.0, 0.0)
        for a0, a1 in zip(inside, inside[1:]):
            hb0, ht0, hb1, ht1 = bottom(a0), along_line(c["top"], a0), bottom(a1), along_line(c["top"], a1)
#WALL
        for a, facing in ((d0, -1.0), (d1, 1.0)):
            points = profile(a)
            for (x0, h0), (x1, h1) in zip(points, points[1:]):
                body.flat.append(([(sx * x0, y_of(a), z(h0)), (sx * x1, y_of(a), z(h1)), (sx * lining, y_of(a), z(h1)), (sx * lining, y_of(a), z(h0))], DARK, (0.0, facing, 0.0)))

    body_obj = body.build()
    # Each door's hinge: on its front seam, on the outer skin and at the foot of the door.
    door_objs = [doors[sx].build((sx * width(d0), y_of(d0), door_foot)) for sx in (-1, 1)]
    print(f"doors {name}: hinge_L (game) [{-width(d0):.3f}, {door_foot:.3f}, {d0 - hz:.3f}] length {d1 - d0:.3f} height {door_top - door_foot:.3f}")
    wheels = styled_wheels(spec, c.get("wheels", "steel"), c.get("rim", 0.62))
    add_seats(to_blender(heads))
    return [body_obj, *door_objs, *wheels]


def lip(body, c, a, h, span, deep=0.14, rx=0.0):
    """A low black spoiler `span` across, `a` back from the front face at height `h` above the ground."""
    body.box((0.0, c["y_of"](a), c["z"](h)), (span, deep, 0.025), DARK, rx=rx)


# Each car's shape: heights above the ground and lengths back from the front face, metres, read off the
# real car at its size. `top` and `glass` are the lines along the top of the lower body and of the
# glasshouse; `doors` the seams down its sides; `posts` the window posts between the windscreen's and the
# rear one.
def build_saab_900():
    def details(body, c):
        # The clamshell bonnet's shut line, a lip on the hatch, and the deep black bumpers' rubbing strip.
        for sx in (-1, 1):
            beam(body, (sx * 0.5, c["y_of"](0.12), c["z"](c["top"](0.12)) + 0.004), (sx * 0.62, c["y_of"](1.3), c["z"](c["top"](1.3)) + 0.004), 0.012, DARK)
        lip(body, c, 4.47, 0.985, 1.3, rx=0.1)
    c = dict(
        sill=0.18, chin=(0.3, 0.3), tail_lift=(0.32, 0.3), nose_round=0.24, tail_round=0.18, tumble=0.15, shoulder=(0.08, 0.05),
        top=[(0, 0.62), (0.12, 0.73), (0.55, 0.83), (1.0, 0.9), (1.32, 0.93), (4.45, 0.97), (4.55, 0.95), (4.66, 0.86)],
        glass=[(1.32, 0.93), (1.45, 1.2), (1.6, 1.36), (1.78, 1.42), (3.05, 1.42), (3.3, 1.39), (3.9, 1.18), (4.45, 0.97)],
        doors=[1.42, 2.62], posts=[2.62], c_post=0.26, b_post=0.1,
        lamps=dict(kind="rect", x=(0.25, 0.62), z=(0.49, 0.61)), grille=dict(x=0.22, z=(0.49, 0.6), slats=3, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.29, hi=0.47, out=0.07), tail=dict(x=(0.28, 0.62), z=(0.62, 0.84), plate=0.72),
        strip=(0.62, DARK, 0.04), wheels="spokes3", rim=0.66,
    )
    return build_saloon("saab_900", c, HEADS["saab_900"], details)


def build_saab_9000():
    c = dict(
        sill=0.18, chin=(0.3, 0.3), tail_lift=(0.32, 0.25), nose_round=0.2, tail_round=0.14, tumble=0.13, shoulder=(0.08, 0.05),
        clad=(0.34, DARK),
        top=[(0, 0.66), (0.1, 0.75), (0.6, 0.86), (1.15, 0.93), (4.45, 0.98), (4.52, 0.97), (4.6, 0.9)],
        glass=[(1.15, 0.93), (1.32, 1.18), (1.5, 1.35), (1.72, 1.42), (3.1, 1.42), (3.35, 1.38), (3.95, 1.2), (4.45, 0.98)],
        doors=[1.22, 2.25, 3.25], posts=[2.25, 3.28], c_post=0.12, b_mat=DARK,
        lamps=dict(kind="rect", x=(0.24, 0.66), z=(0.54, 0.64), fog=((0.55, 0.38),)), grille=dict(x=0.2, z=(0.53, 0.63), slats=2, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.3, hi=0.46, out=0.05), tail=dict(x=(0.16, 0.66), z=(0.68, 0.86), plate=0.58),
        wheels="spokes5", rim=0.68,
    )
    return build_saloon("saab_9000", c, HEADS["saab_9000"])


def volvo_240_shape():
    """The Volvo 240's measurements for `build_saloon`: the police car is built on them too."""
    return dict(
        sill=0.19, chin=(0.32, 0.22), tail_lift=(0.32, 0.22), nose_round=0.08, tail_round=0.07, tumble=0.07, shoulder=(0.05, 0.03), tuck=0.04,
        top=[(0, 0.83), (0.05, 0.87), (1.05, 0.95), (4.75, 0.98), (4.8, 0.97)],
        glass=[(1.05, 0.95), (1.2, 1.2), (1.38, 1.39), (1.62, 1.46), (4.66, 1.46), (4.74, 1.42), (4.79, 1.0)],
        doors=[1.12, 2.15, 3.18], posts=[2.15, 3.2], c_post=0.13, trim=STEEL, handles=STEEL,
        lamps=dict(kind="rect", x=(0.37, 0.7), z=(0.62, 0.79)), grille=dict(x=0.35, z=(0.58, 0.82), slats=4, frame=STEEL),
        indicators="corner",
        bumpers=dict(mat=DARK, lo=0.33, hi=0.52, out=0.14, back=0.38), tail=dict(x=(0.5, 0.74), z=(0.62, 0.9), plate=0.78),
        strip=(0.6, DARK, 0.05), wheels="steel",
    )


def build_volvo_240():
    def details(body, c):
        # Roof rails, and the tall lamps up the tailgate's sides.
        for sx in (-1, 1):
            x = sx * (c["hx"] - 0.2)
            beam(body, (x, c["y_of"](1.75), c["z"](1.47)), (x, c["y_of"](4.6), c["z"](1.47)), 0.035, DARK)
            body.decal((sx * (c["hx"] - 0.12), c["back_face"], c["z"](1.12)), (0.13, 0.36), (0, -1, 0), RED)
    return build_saloon("volvo_240", volvo_240_shape(), HEADS["volvo_240"], details)


def build_volvo_740():
    c = dict(
        sill=0.18, chin=(0.31, 0.22), tail_lift=(0.31, 0.22), nose_round=0.08, tail_round=0.07, tumble=0.08, shoulder=(0.05, 0.03), tuck=0.04,
        top=[(0, 0.8), (0.05, 0.84), (1.15, 0.94), (3.78, 0.96), (4.7, 0.95), (4.8, 0.91)],
        glass=[(1.15, 0.94), (1.3, 1.19), (1.48, 1.36), (1.72, 1.41), (3.3, 1.41), (3.42, 1.37), (3.78, 0.96)],
        doors=[1.22, 2.25, 3.3], posts=[2.25], c_post=0.22, trim=STEEL, handles=STEEL,
        lamps=dict(kind="rect", x=(0.34, 0.72), z=(0.6, 0.75), bars=(0.7,)), grille=dict(x=0.31, z=(0.56, 0.78), slats=4, frame=STEEL),
        bumpers=dict(mat=DARK, lo=0.31, hi=0.5, out=0.12, back=0.38), tail=dict(x=(0.12, 0.76), z=(0.66, 0.87), plate=0.56),
        strip=(0.6, DARK, 0.05), wheels="dial",
    )
    return build_saloon("volvo_740", c, HEADS["volvo_740"])


def build_gti():
    def details(body, c):
        # The red line round the grille, a spoiler over the hatch, and black flares round the arches.
        body.decal((0.0, c["face"] + 0.006, c["z"](0.545)), (0.48, 0.014), (0, 1, 0), RED)
        lip(body, c, 3.2, 1.41, 1.2, deep=0.2, rx=0.12)
        spec = SPECS["gti"]
        inner, outer = spec["wheel_radius"] + 0.055, spec["wheel_radius"] + 0.11
        for at in (c["hz"] - spec["axle"], c["hz"] + spec["axle"]):
            for sx in (-1, 1):
                rings = []
                for d in (0, 45, 90, 135, 180):
                    t = math.radians(d)
                    at_r = lambda r: (c["y_of"](at - r * math.cos(t)), c["z"](spec["wheel_radius"] + r * math.sin(t)))
                    (y0, z0), (y1, z1) = at_r(inner), at_r(outer)
                    x0, x1 = sx * (c["hx"] - 0.01), sx * (c["hx"] + 0.025)
                    rings.append([(x0, y0, z0), (x1, y0, z0), (x1, y1, z1), (x0, y1, z1)])
                shell(body, rings, lambda s, k: None if k == 3 else DARK, cap=DARK)
    c = dict(
        sill=0.17, chin=(0.27, 0.25), tail_lift=(0.29, 0.22), nose_round=0.13, tail_round=0.12, tumble=0.15, shoulder=(0.07, 0.04),
        top=[(0, 0.67), (0.08, 0.75), (0.5, 0.86), (0.95, 0.93), (3.86, 0.96), (3.92, 0.95), (4.0, 0.87)],
        glass=[(0.95, 0.93), (1.12, 1.17), (1.3, 1.34), (1.52, 1.41), (3.12, 1.41), (3.25, 1.37), (3.86, 0.96)],
        doors=[1.0, 2.32], posts=[2.32], c_post=0.36, b_mat=DARK,
        lamps=dict(kind="round2", x=(0.27, 0.68), z=(0.53, 0.66)), grille=dict(x=0.26, z=(0.54, 0.65), slats=3, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.27, hi=0.44, out=0.05), tail=dict(x=(0.34, 0.68), z=(0.62, 0.8), plate=0.71, panel=DARK),
        strip=(0.48, DARK, 0.05), wheels="dial", rim=0.66,
    )
    return build_saloon("gti", c, HEADS["gti"], details)


def build_transporter():
    def details(body, c):
        hx = c["hx"]
        # The sliding door's track and seams on the right, the vents behind the back side windows, and
        # the second grille under the windscreen.
        body.decal((hx + 0.004, c["y_of"](2.35), c["z"](1.09)), (1.4, 0.03), (1, 0, 0), DARK)
        mirrored_decal(body, hx + 0.003, c["y_of"](3.75), c["z"](0.95), (0.32, 0.12), (1, 0, 0), DARK)
        for k in range(3):
            mirrored_decal(body, hx + 0.005, c["y_of"](3.75), c["z"](0.92 + 0.03 * k), (0.3, 0.01), (1, 0, 0), PAINT)
        body.decal((0.0, c["face"] + 0.002, c["z"](0.99)), (1.4, 0.06), (0, 1, 0), DARK)
    return build_saloon("transporter", transporter_shape(), HEADS["transporter"], details)


def transporter_shape():
    """The Transporter's measurements for `build_saloon`: the ambulance is built on them too."""
    return dict(
        sill=0.29, chin=(0.38, 0.15), tail_lift=(0.38, 0.15), nose_round=0.12, tail_round=0.1, tumble=0.07, shoulder=(0.04, 0.03), tuck=0.04,
        upper=WHITE,
        top=[(0, 1.02), (0.05, 1.06), (4.5, 1.07)],
        glass=[(0.06, 1.06), (0.2, 1.45), (0.36, 1.82), (0.6, 1.95), (4.4, 1.95), (4.48, 1.9), (4.5, 1.1)],
        doors=[0.08, 1.05, 1.65, 2.95], posts=[1.05, 2.25, 3.35], c_post=0.12, b_post=0.09,
        lamps=dict(kind="rect", x=(0.52, 0.82), z=(0.78, 0.92)), grille=dict(x=0.48, z=(0.79, 0.91), slats=3, slat=DARK),
        indicators="below",
        bumpers=dict(mat=DARK, lo=0.4, hi=0.56, out=0.06), tail=dict(x=(0.7, 0.86), z=(0.6, 1.0), plate=0.72),
        wheels="steel", rim=0.6,
    )


def build_kadett():
    def details(body, c):
        lip(body, c, 3.9, 0.955, 1.3, deep=0.1)
    c = dict(
        sill=0.17, chin=(0.27, 0.25), tail_lift=(0.29, 0.22), nose_round=0.17, tail_round=0.12, tumble=0.15, shoulder=(0.07, 0.04),
        top=[(0, 0.6), (0.12, 0.7), (0.5, 0.8), (0.95, 0.9), (3.88, 0.94), (3.93, 0.93), (4.0, 0.86)],
        glass=[(0.95, 0.9), (1.18, 1.14), (1.42, 1.32), (1.72, 1.4), (2.95, 1.4), (3.2, 1.34), (3.88, 0.95)],
        doors=[1.0, 2.05, 2.98], posts=[2.05], c_post=0.24, b_mat=DARK,
        lamps=dict(kind="rect", x=(0.27, 0.66), z=(0.5, 0.59)), grille=dict(x=0.25, z=(0.51, 0.58), slats=1, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.27, hi=0.45, out=0.05), tail=dict(x=(0.3, 0.66), z=(0.6, 0.8), plate=0.7, panel=DARK),
        strip=(0.5, DARK, 0.04), wheels="steel",
    )
    return build_saloon("kadett", c, HEADS["kadett"], details)


def build_omega():
    c = dict(
        sill=0.18, chin=(0.27, 0.25), tail_lift=(0.3, 0.22), nose_round=0.2, tail_round=0.14, tumble=0.14, shoulder=(0.07, 0.05),
        top=[(0, 0.62), (0.12, 0.72), (0.6, 0.83), (1.1, 0.91), (3.8, 0.94), (4.5, 0.97), (4.58, 0.96), (4.66, 0.9)],
        glass=[(1.1, 0.91), (1.32, 1.15), (1.55, 1.35), (1.85, 1.43), (3.05, 1.43), (3.25, 1.39), (3.8, 0.95)],
        doors=[1.16, 2.3, 3.36], posts=[2.3], c_post=0.18, b_mat=DARK,
        lamps=dict(kind="rect", x=(0.24, 0.7), z=(0.52, 0.61)), grille=dict(x=0.22, z=(0.53, 0.6), slats=2),
        bumpers=dict(mat=PAINT, lo=0.27, hi=0.46, out=0.04), tail=dict(x=(0.14, 0.74), z=(0.72, 0.88), plate=0.58),
        strip=(0.5, DARK, 0.04), wheels="spokes6", rim=0.66,
    )
    return build_saloon("omega", c, HEADS["omega"])


def build_sierra():
    def details(body, c):
        # No grille, but a black panel across the nose with the lamps in it.
        body.decal((0.0, c["face"] + 0.001, c["z"](0.545)), (2 * (c["hx"] - c["nose_round"]) - 0.02, 0.1), (0, 1, 0), DARK)
    c = dict(
        sill=0.18, chin=(0.26, 0.28), tail_lift=(0.3, 0.25), nose_round=0.26, tail_round=0.2, tumble=0.15, shoulder=(0.08, 0.05),
        clad=(0.36, DARK),
        top=[(0, 0.6), (0.15, 0.71), (0.6, 0.83), (1.0, 0.9), (4.28, 0.95), (4.33, 0.94), (4.4, 0.86)],
        glass=[(1.0, 0.9), (1.22, 1.15), (1.45, 1.33), (1.78, 1.4), (2.95, 1.4), (3.25, 1.34), (3.8, 1.14), (4.28, 0.96)],
        doors=[1.06, 2.16, 3.16], posts=[2.16, 3.2], c_post=0.1, b_mat=DARK,
        lamps=dict(kind="rect", x=(0.22, 0.6), z=(0.5, 0.59)), grille=dict(x=0.2, z=(0.51, 0.58), slats=2, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.26, hi=0.45, out=0.05), tail=dict(x=(0.3, 0.6), z=(0.6, 0.82), plate=0.72),
        wheels="steel",
    )
    return build_saloon("sierra", c, HEADS["sierra"], details)


def build_escort():
    c = dict(
        sill=0.17, chin=(0.28, 0.22), tail_lift=(0.29, 0.2), nose_round=0.1, tail_round=0.1, tumble=0.14, shoulder=(0.07, 0.04),
        top=[(0, 0.66), (0.08, 0.72), (0.95, 0.9), (3.72, 0.91), (3.92, 0.92), (4.0, 0.86)],
        glass=[(0.95, 0.9), (1.15, 1.13), (1.35, 1.3), (1.62, 1.38), (2.85, 1.38), (3.05, 1.32), (3.72, 0.93)],
        doors=[1.0, 2.05, 2.92], posts=[2.05], c_post=0.2,
        lamps=dict(kind="rect", x=(0.31, 0.66), z=(0.52, 0.64)), grille=dict(x=0.29, z=(0.52, 0.63), slats=4, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.28, hi=0.45, out=0.05), tail=dict(x=(0.3, 0.66), z=(0.62, 0.84), plate=0.74),
        strip=(0.5, DARK, 0.04), wheels="steel",
    )
    return build_saloon("escort", c, HEADS["escort"])


def build_sports():
    """The sports car, an Opel Manta GT/E: a long, low bonnet, a short coupé roof with no B post, a
    short boot, black bumpers and a chin spoiler."""
    def details(body, c):
        body.box((0.0, c["y_of"](0.05), c["z"](0.2)), (2 * c["hx"] - 0.2, 0.18, 0.05), DARK)
        lip(body, c, 4.33, 0.875, 1.35, deep=0.1)
    c = dict(
        sill=0.15, chin=(0.24, 0.3), tail_lift=(0.27, 0.25), nose_round=0.14, tail_round=0.15, tumble=0.17, shoulder=(0.07, 0.05),
        top=[(0, 0.58), (0.08, 0.65), (0.7, 0.76), (1.45, 0.84), (3.72, 0.86), (4.3, 0.86), (4.4, 0.8)],
        glass=[(1.45, 0.84), (1.7, 1.06), (1.95, 1.24), (2.2, 1.3), (2.95, 1.3), (3.15, 1.25), (3.72, 0.87)],
        doors=[1.5, 2.78], posts=[], c_post=0.3,
        lamps=dict(kind="rect", x=(0.28, 0.62), z=(0.45, 0.56)), grille=dict(x=0.26, z=(0.45, 0.55), slats=3, slat=DARK),
        bumpers=dict(mat=DARK, lo=0.26, hi=0.38, out=0.05), tail=dict(x=(0.27, 0.64), z=(0.6, 0.78), plate=0.69),
        strip=(0.48, DARK, 0.03), wheels="spokes5", rim=0.68,
    )
    return build_saloon("sports", c, HEADS["sports"], details)


def build_jeep():
    """The jeep, a Ford Bronco of the early eighties with its top off: a tall, square tub open over the
    seats, a windscreen frame, a roll bar over the front seats, chrome bumpers and a spare on the tailgate."""
    spec = SPECS["jeep"]
    hx, _, hz = spec["half"]
    ground = -spec["ride"]
    sill, floor, nose, belt, tail = 0.5 + ground, 0.82 + ground, 1.2 + ground, 1.32 + ground, 1.3 + ground
    cowl = 0.85
    arch_top = 2 * spec["wheel_radius"] + ground + 0.05
    body = Part("body")
    # The floor of the tub, the bonnet, and the tub's sides up to the waist, all cut round the wheels.
    round_wheels(body, spec, [(-hz, sill), (hz - 0.04, sill), (hz, sill + 0.06), (hz, floor), (-hz, floor)], -hx, hx, PAINT, arch_top)
    round_wheels(body, spec, [(cowl, floor), (hz, floor), (hz, nose - 0.03), (hz - 0.04, nose), (cowl, nose + 0.04)], -hx, hx, PAINT, arch_top)
    for side in (-1, 1):
        x0, x1 = sorted((side * hx, side * (hx - 0.08)))
        round_wheels(body, spec, [(-hz, floor), (cowl, floor), (cowl, belt), (-hz, belt)], x0, x1, PAINT, arch_top)
    wheel_wells(body, spec, sill, arch_top)
    # The tailgate, the dash, and black flares over the arches.
    body.box((0.0, -hz + 0.04, (floor + tail) / 2), (2 * hx - 0.16, 0.08, tail - floor), PAINT)
    body.box((0.0, cowl - 0.08, (nose + belt) / 2 + 0.02), (2 * hx - 0.16, 0.16, belt - nose + 0.12), DARK)
    for y in (-spec["axle"], spec["axle"]):
        mirrored(body, hx + 0.02, y, arch_top + 0.03, (0.06, 2 * spec["wheel_radius"] + 0.2, 0.06), DARK)
    seats_for(body, to_blender([(-0.4, 0.62, -0.3), (0.4, 0.62, -0.3), (-0.4, 0.62, 0.75), (0.4, 0.62, 0.75)]))
    # The windscreen in its frame, and the roll bar behind the front seats.
    top, lean = belt + 0.48, 0.22
    body.box((0.0, cowl - lean / 2, (belt + top) / 2), (2 * hx - 0.2, 0.03, math.hypot(top - belt, lean)), GLASS, rx=math.atan2(lean, top - belt))
    for side in (-1, 1):
        beam(body, (side * (hx - 0.08), cowl, belt), (side * (hx - 0.08), cowl - lean, top), 0.07, PAINT)
    beam(body, (-(hx - 0.08), cowl - lean, top), (hx - 0.08, cowl - lean, top), 0.07, PAINT)
    hoop = -0.15
    for side in (-1, 1):
        beam(body, (side * (hx - 0.1), hoop, belt), (side * (hx - 0.12), hoop - 0.05, belt + 0.52), 0.08, STEEL)
        beam(body, (side * (hx - 0.1), hoop, belt + 0.2), (side * (hx - 0.1), -hz + 0.2, belt), 0.06, STEEL)
    beam(body, (-(hx - 0.12), hoop - 0.05, belt + 0.52), (hx - 0.12, hoop - 0.05, belt + 0.52), 0.08, STEEL)
    # The nose: a chrome bumper, a square grille between round lamps, and amber indicators under them.
    low = sill + 0.1
    body.box((0.0, hz + 0.08, low), (2 * hx + 0.06, 0.1, 0.16), STEEL)
    body.box((0.0, hz + 0.005, (low + nose) / 2 + 0.05), (1.0, 0.03, 0.3), DARK)
    for side in (-1, 1):
        body.cylinder((side * 0.72, hz + 0.01, (low + nose) / 2 + 0.06), 0.11, 0.03, LAMP, axis="Y", segments=10)
        body.box((side * 0.72, hz + 0.01, low + 0.14), (0.14, 0.03, 0.06), AMBER)
    # The tail: a chrome bumper, the lamps, the plate, and the spare on the tailgate.
    body.box((0.0, -hz - 0.06, low), (2 * hx + 0.06, 0.1, 0.16), STEEL)
    mirrored(body, hx - 0.1, -hz - 0.008, (floor + tail) / 2, (0.12, 0.03, 0.3), RED)
    body.box((0.0, -hz - 0.01, floor + 0.08), (0.5, 0.02, 0.12), WHITE)
    body.cylinder((0.0, -hz - 0.14, (floor + tail) / 2 + 0.1), 0.45, 0.22, DARK, axis="Y", segments=12)
    body.cylinder((0.0, -hz - 0.26, (floor + tail) / 2 + 0.1), 0.26, 0.04, STEEL, axis="Y", segments=8)
    # Mirrors on the windscreen frame.
    mirrored(body, hx + 0.08, cowl - 0.05, belt + 0.1, (0.12, 0.05, 0.12), DARK)
    body_obj = body.build()
    wheels = add_wheels(spec, lugs=12, rim_ratio=0.6)
    add_seats(to_blender([(-0.4, 0.62, -0.3), (0.4, 0.62, -0.3), (-0.4, 0.62, 0.75), (0.4, 0.62, 0.75)]))
    return [body_obj, *wheels]


# --------------------------------------------------------------------------------------------------
# The emergency vehicles (VEHICLES-XXV.md): the police car on the Volvo 240's body and the ambulance on
# the Transporter's, each in its livery with a light bar on the roof. The bar's two lenses are nodes
# of their own, `beacon_L` and `beacon_R`, beside the body and not under it, so the game can light
# each by name; their origins are the middles of the lenses, where their light shines from.
# --------------------------------------------------------------------------------------------------
# Block capitals, five across and seven down, for the lettering down a side.
GLYPHS = {
    "A": (".###.", "#...#", "#...#", "#####", "#...#", "#...#", "#...#"),
    "B": ("####.", "#...#", "#...#", "####.", "#...#", "#...#", "####."),
    "I": ("#####", "..#..", "..#..", "..#..", "..#..", "..#..", "#####"),
    "L": ("#....", "#....", "#....", "#....", "#....", "#....", "#####"),
    "M": ("#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#", "#...#"),
    "N": ("#...#", "##..#", "#.#.#", "#..##", "#...#", "#...#", "#...#"),
    "O": (".###.", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
    "P": ("####.", "#...#", "#...#", "####.", "#....", "#....", "#...."),
    "S": (".####", "#....", "#....", ".###.", "....#", "....#", "####."),
    "U": ("#...#", "#...#", "#...#", "#...#", "#...#", "#...#", ".###."),
}


def livery(name, rgb):
    """A colour only the model being built wears, after the shared materials: its index for a part's
    faces. `reset_scene` starts every model without it, so no other model is changed by it. Made before
    any part of the model is built."""
    MATERIALS.append(common.material(name, rgb))
    return len(MATERIALS) - 1


def lettering(part, text, middle, right, up, height, mat):
    """`text` in block capitals `height` tall, as flat panels about `middle`: it reads along `right`
    with `up` its top, and faces along right x up, so on a car's left side `right` is towards the tail."""
    right, up = Vector(right).normalized(), Vector(up).normalized()
    normal = tuple(right.cross(up))
    dot = height / 7
    start = Vector(middle) - right * (dot * (6 * len(text) - 1) / 2) - up * (height / 2)
    for n, letter in enumerate(text):
        # Each run of dots along a row, joined with the same run in the rows under it: a few panels a letter.
        runs = {}
        for r, row in enumerate(GLYPHS[letter]):
            c = 0
            while c < 5:
                if row[c] != "#":
                    c += 1
                    continue
                c1 = c
                while c1 + 1 < 5 and row[c1 + 1] == "#":
                    c1 += 1
                above = runs.pop((r - 1, c, c1), None)
                runs[(r, c, c1)] = r if above is None else above
                c = c1 + 1
        for (r1, c0, c1), r0 in runs.items():
            x0, x1 = (6 * n + c0) * dot, (6 * n + c1 + 1) * dot
            y0, y1 = (6 - r1) * dot, (7 - r0) * dot
            part.flat.append(([tuple(start + right * x + up * y) for x, y in ((x0, y0), (x1, y0), (x1, y1), (x0, y1))], mat, normal))


def light_bar(body, y, roof, half, lens, built):
    """A light bar across the roof at `y`, standing on it at height `roof` (Blender z) and reaching
    `half` to each side: a black foot, the siren's horn in the middle and a lens either side of it. The
    lenses are their own nodes, `beacon_L` (the driver's side) and `beacon_R`, in material `lens`, added
    to `built`."""
    body.box((0.0, y, roof + 0.015), (2 * half + 0.06, 0.3, 0.05), DARK)
    body.box((0.0, y, roof + 0.1), (0.3, 0.24, 0.12), STEEL)
    body.decal((0.0, y + 0.122, roof + 0.1), (0.24, 0.08), (0, 1, 0), DARK)
    span = half - 0.15
    for tag, sx in (("L", -1), ("R", 1)):
        beacon = Part(f"beacon_{tag}", location=(sx * (0.15 + span / 2), y, roof + 0.11))
        beacon.box((0.0, 0.0, 0.0), (span, 0.26, 0.14), lens)
        built.append(beacon.build())


def build_police_car():
    blue, lens = livery("blue", (0.05, 0.16, 0.5)), livery("beacon", (0.08, 0.25, 0.9))
    beacons = []

    def details(body, c):
        hx, z, y_of = c["hx"], c["z"], c["y_of"]
        # The Volvo's tall lamps up the tailgate's sides.
        for sx in (-1, 1):
            body.decal((sx * (hx - 0.12), c["back_face"], z(1.12)), (0.13, 0.36), (0, -1, 0), RED)
        light_bar(body, y_of(2.15), z(c["roof"]), 0.55, lens, beacons)
        # A blue stripe down each side over the arches, and POLIS on the front doors and the bonnet.
        # The lettering fits between the front arch and the door's back seam.
        a0, a1 = 0.12, 2 * c["hz"] - 0.12
        mirrored_decal(body, hx + 0.004, y_of((a0 + a1) / 2), z(0.76), (a1 - a0, 0.12), (1, 0, 0), blue)
        lettering(body, "POLIS", (-hx - 0.004, y_of(1.81), z(0.5)), (0, -1, 0), (0, 0, 1), 0.16, blue)
        lettering(body, "POLIS", (hx + 0.004, y_of(1.81), z(0.5)), (0, 1, 0), (0, 0, 1), 0.16, blue)
        # Read from in front of the car: its top is towards the windscreen, up the bonnet's slope.
        slope = Vector((0.0, -0.9, c["top"](1.0) - c["top"](0.1))).normalized()
        lettering(body, "POLIS", Vector((0.0, y_of(0.55), z(c["top"](0.55)))) + Vector((-1, 0, 0)).cross(slope) * 0.006, (-1, 0, 0), slope, 0.24, blue)

    # No rubbing strip: the lettering is where it ran.
    objects = build_saloon("police_car", dict(volvo_240_shape(), strip=None), HEADS["police_car"], details)
    return [*objects, *beacons]


def build_ambulance():
    lens = livery("beacon", (0.08, 0.25, 0.9))
    beacons = []

    def details(body, c):
        hx, z, y_of = c["hx"], c["z"], c["y_of"]
        # The sliding door's track on the right and the second grille under the windscreen, as the van's.
        body.decal((hx + 0.004, y_of(2.35), z(1.09)), (1.4, 0.03), (1, 0, 0), DARK)
        body.decal((0.0, c["face"] + 0.002, z(0.99)), (1.4, 0.06), (0, 1, 0), DARK)
        light_bar(body, y_of(0.95), z(c["roof"]), 0.6, lens, beacons)
        # A red stripe down each side under the windows, and AMBULANS on the panels between the cab's
        # doors and the back wheels.
        a0, a1 = 0.15, 2 * c["hz"] - 0.15
        mirrored_decal(body, hx + 0.004, y_of((a0 + a1) / 2), z(0.84), (a1 - a0, 0.1), (1, 0, 0), RED)
        lettering(body, "AMBULANS", (-hx - 0.004, y_of(2.3), z(0.61)), (0, -1, 0), (0, 0, 1), 0.18, RED)
        lettering(body, "AMBULANS", (hx + 0.004, y_of(2.3), z(0.61)), (0, 1, 0), (0, 0, 1), 0.18, RED)

    objects = build_saloon("ambulance", transporter_shape(), HEADS["ambulance"], details)
    return [*objects, *beacons]


def build_police_van():
    blue, lens = livery("blue", (0.05, 0.16, 0.5)), livery("beacon", (0.08, 0.25, 0.9))
    beacons = []

    def details(body, c):
        hx, z, y_of = c["hx"], c["z"], c["y_of"]
        # The sliding door's track on the right and the second grille under the windscreen, as the van's.
        body.decal((hx + 0.004, y_of(2.35), z(1.09)), (1.4, 0.03), (1, 0, 0), DARK)
        body.decal((0.0, c["face"] + 0.002, z(0.99)), (1.4, 0.06), (0, 1, 0), DARK)
        light_bar(body, y_of(0.95), z(c["roof"]), 0.6, lens, beacons)
        # The police car's blue stripe down each side under the windows, and POLIS on the panels
        # between the cab's doors and the back wheels.
        a0, a1 = 0.15, 2 * c["hz"] - 0.15
        mirrored_decal(body, hx + 0.004, y_of((a0 + a1) / 2), z(0.84), (a1 - a0, 0.12), (1, 0, 0), blue)
        lettering(body, "POLIS", (-hx - 0.004, y_of(2.3), z(0.58)), (0, -1, 0), (0, 0, 1), 0.22, blue)
        lettering(body, "POLIS", (hx + 0.004, y_of(2.3), z(0.58)), (0, 1, 0), (0, 0, 1), 0.22, blue)

    objects = build_saloon("police_van", transporter_shape(), HEADS["police_van"], details)
    return [*objects, *beacons]


BUILDERS = {"amv": build_amv, "bus": build_bus, "jeep": build_jeep, "sports": build_sports, "roadster": build_roadster, "snowmobile": build_snowmobile, "golf_cart": build_golf_cart, "technical": build_technical, "snowracer": build_snowracer, "epa": build_epa, "saab_900": build_saab_900, "saab_9000": build_saab_9000, "volvo_240": build_volvo_240, "volvo_740": build_volvo_740, "gti": build_gti, "transporter": build_transporter, "kadett": build_kadett, "omega": build_omega, "sierra": build_sierra, "escort": build_escort, "police_car": build_police_car, "ambulance": build_ambulance, "police_van": build_police_van}


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
        # The front doors swung open `--open` radians about their hinges for the pictures (only after
        # the export): each one's back edge out, away from the car.
        for tag, sx in (("FL", -1), ("FR", 1)):
            door = bpy.data.objects.get(f"door_{tag}")
            if door and OPEN:
                door.rotation_euler.z = sx * float(OPEN)
        preview(PREVIEW, name)
