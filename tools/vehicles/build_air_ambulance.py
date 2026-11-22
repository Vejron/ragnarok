"""
Builds the air ambulance (an EC135 in Swedish ambulance yellow) and exports it to
client/public/models/air_ambulance.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_air_ambulance.py -- \
        [--out client/public/models/air_ambulance.glb] [--preview DIR]

Everything is procedural and flat-shaded, under 3,500 triangles in all, markings included: the lettering,
the chequers and the Stars of Life are flat quads just off the skin, so there are no textures.

Conventions
  * Blender: Z up, the nose points along +Y, the right hand side is +X. The glTF exporter turns that into
    the game's vehicle frame (+X right, +Y up, -Z forward), so a game point (x, y, z) is (x, -z, y) here.
  * Origin is the centre of the hull box (1.7 x 2.2 x 5.1 m). The skids' soles are 1.1 m below it.
  * The cabin is a hollow shell with glass for windows, so whoever sits in it shows. It has an inner skin,
    since the game draws only the fronts of faces.
  * Nodes the game looks up by name:
      `hull`        everything fixed to the airframe, with the `cabin` as its child
      `rotor`       origin on the rotor head: the game spins it about Y
      `tail_rotor`  origin on the tail fan's hub: the game spins it about X
      `seat_0..3`   each seat's head point (AIR_AMBULANCE_SEAT_HEADS)
  * Materials: `glass` is drawn translucent; the others keep their colours, and are all one draw a node.
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
from common import MATERIALS, about_x, about_y, about_z, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/air_ambulance.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with AIR_AMBULANCE_* in crates/shared/src/vehicles/.
# --------------------------------------------------------------------------------------------------
GROUND = -1.1
ROTOR_HEAD = Vector((0.0, -0.7, 2.1))  # game [0, 2.1, 0.7]
ROTOR_RADIUS = 5.1
FAN_HUB = Vector((0.0, -6.55, 0.65))  # game [0, 0.65, 6.55]
LAMP = Vector((0.0, 2.1, -0.5))  # game [0, -0.5, -2.1]
SEAT_HEADS = [(0.31, 0.85, 0.72), (-0.31, 0.85, 0.72), (-0.31, -0.35, 0.74), (0.31, -0.35, 0.74)]

# --------------------------------------------------------------------------------------------------
# Scene and materials (linear colours)
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

MATERIALS[:] = [
    material("livery", (0.72, 0.8, 0.035)),
    material("dark", (0.025, 0.027, 0.03)),
    material("steel", (0.46, 0.48, 0.48)),
    material("glass", (0.3, 0.45, 0.52)),
    material("green", (0.0, 0.14, 0.055)),
    material("blue", (0.01, 0.09, 0.55)),
    material("red", (0.65, 0.04, 0.035)),
    material("lining", (0.2, 0.21, 0.2)),
    material("white", (0.85, 0.85, 0.82)),
]
LIVERY, DARK, STEEL, GLASS, GREEN, BLUE, RED, LINING, WHITE = range(9)


class Part(common.Part):
    def box(self, centre, size, mat, rot=None):
        """A box, turned by `rot` (a 4x4 rotation) about its own centre."""
        turn = rot if rot is not None else Matrix.Identity(4)
        m = Matrix.Translation(centre) @ turn @ Matrix.Diagonal((*size, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def tube(self, start, end, radius, mat, segments=5):
        """A straight tube from `start` to `end`."""
        start, end = Vector(start), Vector(end)
        axis = end - start
        m = Matrix.Translation((start + end) / 2) @ axis.to_track_quat("Z", "Y").to_matrix().to_4x4()
        verts = bmesh.ops.create_cone(self.bm, cap_ends=True, cap_tris=False, segments=segments, radius1=radius, radius2=radius, depth=axis.length, matrix=m)["verts"]
        self._tag(verts, mat)


class Skin(common.Part):
    """Faces wound by hand, which `build` leaves as they are: an open shell has no outside to find."""

    def face(self, points, mat, out):
        """A face looking along `out`, whichever way round its points come."""
        corners = []
        for point in map(Vector, points):
            if all((point - kept).length > 1e-5 for kept in corners):
                corners.append(point)
        if len(corners) < 3:
            return
        normal = Vector()
        for k in range(1, len(corners) - 1):
            normal += (corners[k] - corners[0]).cross(corners[k + 1] - corners[0])
        if normal.dot(Vector(out)) < 0:
            corners.reverse()
        self.bm.faces.new([self.bm.verts.new(point) for point in corners]).material_index = mat

    def build(self):
        mesh = bpy.data.meshes.new(self.name)
        self.bm.to_mesh(mesh)
        self.bm.free()
        for m in MATERIALS:
            mesh.materials.append(m)
        obj = bpy.data.objects.new(self.name, mesh)
        obj.parent = self.parent
        scene.collection.objects.link(obj)
        return obj


# --------------------------------------------------------------------------------------------------
# The cabin: rings from the nose to where the boom leaves it. Each ring is the right half, mirrored.
# --------------------------------------------------------------------------------------------------
# (y, belly, the side's lower edge, the windows' foot, the windows' head, roof, half width)
STATIONS = [
    (2.64, 0.06, 0.09, 0.13, 0.17, 0.21, 0.07),
    (2.48, -0.18, -0.06, 0.14, 0.34, 0.44, 0.38),
    (2.1, -0.4, -0.22, 0.17, 0.58, 0.74, 0.62),
    (1.6, -0.53, -0.34, 0.22, 0.83, 1.02, 0.76),
    (1.4, -0.56, -0.35, 0.23, 0.92, 1.12, 0.8),
    (1.27, -0.57, -0.36, 0.23, 0.96, 1.18, 0.8),
    (0.95, -0.58, -0.36, 0.24, 1.04, 1.27, 0.8),
    (0.42, -0.6, -0.36, 0.25, 1.07, 1.34, 0.8),
    (0.3, -0.6, -0.36, 0.25, 1.07, 1.35, 0.8),
    (-0.5, -0.6, -0.36, 0.27, 1.06, 1.36, 0.8),
    (-0.72, -0.59, -0.36, 0.3, 1.05, 1.36, 0.8),
    (-1.28, -0.56, -0.36, 0.42, 0.98, 1.36, 0.8),
    (-1.42, -0.54, -0.36, 0.5, 0.9, 1.36, 0.8),
    (-1.75, -0.45, -0.3, 0.5, 0.88, 1.34, 0.78),
    (-2.35, -0.1, 0.06, 0.52, 0.84, 1.25, 0.64),
    (-2.9, 0.28, 0.38, 0.6, 0.8, 1.06, 0.42),
    (-3.3, 0.48, 0.54, 0.65, 0.78, 0.92, 0.2),
]
# The bands between a ring's points, from the belly's middle round to the roof's.
BELLY, BILGE, SIDE, SILL, WINDOW, HEADER, SHOULDER, ROOF, POST = range(9)


def ring(station):
    y, belly, low, foot, head, roof, w = station
    tall = head - foot
    return [
        Vector(p)
        for p in (
            (0.0, y, belly),
            (0.62 * w, y, belly),
            (w, y, low),
            (w, y, foot),
            (0.995 * w, y, foot + 0.1 * tall),
            (0.9 * w, y, head - 0.08 * tall),
            (0.86 * w, y, head),
            (0.58 * w, y, roof - 0.15 * (roof - head)),
            (0.06 * w, y, roof),
            (0.0, y, roof),
        )
    ]


def skin_of(band, front, back):
    """What the skin is made of in `band` between stations at `front` and `back`."""
    y = (front + back) / 2
    if band == SIDE:
        return DARK if 2.1 < y < 2.48 else LIVERY  # the black panel either side of the nose
    if band == SILL:
        return DARK if -1.42 < y < 2.48 else LIVERY
    if band == WINDOW:
        if y > 2.48 or y < -1.42:
            return LIVERY
        panes = ((1.4, 2.48), (0.42, 1.27), (-0.5, 0.3), (-1.28, -0.72))
        return GLASS if any(rear < y < fore for rear, fore in panes) else DARK
    if band in (HEADER, SHOULDER, ROOF, POST) and 0.95 < y < 2.48:
        return DARK if band == POST else GLASS  # the windscreen, with a post up its middle
    if band == HEADER and -1.42 < y <= 0.95:
        return DARK
    return LIVERY


def lined(point, station):
    """A point of the skin drawn in to where the lining is."""
    _, belly, _, _, _, roof, _ = station
    middle = (belly + roof) / 2
    return Vector((point.x * 0.95, point.y, middle + (point.z - middle) * 0.95))


cabin = Skin("cabin")
for fore, aft in zip(STATIONS, STATIONS[1:]):
    a, b = ring(fore), ring(aft)
    for band in range(len(a) - 1):
        mat = skin_of(band, fore[0], aft[0])
        corners = [a[band], a[band + 1], b[band + 1], b[band]]
        middle = sum(corners, Vector()) / 4
        out = (a[band + 1] - a[band] + b[band + 1] - b[band]).cross(b[band] - a[band] + b[band + 1] - a[band + 1])
        if out.length < 1e-6:
            out = Vector((middle.x, 0.0, middle.z - 0.4))
        for side in (1, -1):
            flip = Vector((side, 1, 1))
            cabin.face([p * flip for p in corners], mat, out * flip)
            # The lining is only where someone looking in through the glass can see it.
            if mat != GLASS and aft[0] >= -1.42 and fore[0] <= 2.48:
                within = [lined(p, fore) for p in corners[:2]] + [lined(p, aft) for p in corners[2:]]
                cabin.face([p * flip for p in within], LINING, -out * flip)
for station, out in ((STATIONS[0], (0, 1, 0)), (STATIONS[-1], (0, -1, 0))):
    half = ring(station)
    cabin.face(half + [p * Vector((-1, 1, 1)) for p in half[-2:0:-1]], LIVERY, out)
# The bulkhead behind the rear seats.
rear = STATIONS[12]
half = [lined(p, rear) for p in ring(rear)]
cabin.face(half + [p * Vector((-1, 1, 1)) for p in half[-2:0:-1]], LINING, (0, 1, 0))


# --------------------------------------------------------------------------------------------------
# The markings, laid on the skin
# --------------------------------------------------------------------------------------------------
def skin_x(y, z):
    """How far out the cabin's side is at (y, z)."""
    for fore, aft in zip(STATIONS, STATIONS[1:]):
        if aft[0] <= y <= fore[0]:
            t = (fore[0] - y) / (fore[0] - aft[0])
            points = [p.lerp(q, t) for p, q in zip(ring(fore), ring(aft))]
            for p, q in zip(points[1:7], points[2:7]):
                if p.z <= z <= q.z:
                    return p.x + (q.x - p.x) * (z - p.z) / max(q.z - p.z, 1e-6)
    raise ValueError(f"no skin at {y}, {z}")


def marking(points, mat, side, x=None):
    """A flat patch through the (y, z) `points` on `side` of the cabin, or of whatever is `x` out."""
    cabin.face([(side * ((skin_x(y, z) if x is None else x) + 0.012), y, z) for y, z in points], mat, (side, 0, 0))


def stroke(start, end, width):
    """The four corners of a line from `start` to `end`, squared off past its ends."""
    start, end = Vector(start), Vector(end)
    along = (end - start).normalized() * (width / 2)
    across = Vector((-along.y, along.x))
    return [start - along - across, end + along - across, end + along + across, start - along + across]


def star_of_life(centre, radius, side, x=None):
    """Its three bars, each a hair further out than the last so that they do not fight where they cross."""
    out = skin_x(*centre) if x is None else x
    for k in range(3):
        arm = Vector((math.sin(k * math.pi / 3), math.cos(k * math.pi / 3))) * radius * 0.82
        marking([tuple(p) for p in stroke(Vector(centre) - arm, Vector(centre) + arm, radius * 0.4)], BLUE, side, out + 0.002 * k)


# Each letter's width over its height, and its strokes in a box one high.
LETTERS = {
    "A": (0.74, [[(0, 0), (0.37, 1), (0.74, 0)], [(0.18, 0.36), (0.56, 0.36)]]),
    "M": (0.82, [[(0, 0), (0, 1), (0.41, 0.25), (0.82, 1), (0.82, 0)]]),
    "B": (0.62, [[(0, 0), (0, 1), (0.52, 1), (0.6, 0.78), (0.5, 0.52), (0, 0.52)], [(0.5, 0.52), (0.62, 0.26), (0.52, 0), (0, 0)]]),
    "U": (0.64, [[(0, 1), (0, 0.15), (0.16, 0), (0.48, 0), (0.64, 0.15), (0.64, 1)]]),
    "L": (0.54, [[(0, 1), (0, 0), (0.54, 0)]]),
    "N": (0.64, [[(0, 0), (0, 1), (0.64, 0), (0.64, 1)]]),
    "S": (0.6, [[(0.6, 1), (0.08, 1), (0, 0.62), (0.6, 0.4), (0.52, 0), (0, 0)]]),
}
WORD, TEXT_HIGH, TEXT_FOOT, TEXT_GAP = "AMBULANS", 0.195, -0.05, 0.3
text_width = TEXT_HIGH * (sum(LETTERS[c][0] for c in WORD) + TEXT_GAP * (len(WORD) - 1))
TEXT_MIDDLE = 0.27

for side in (-1, 1):
    # Read from outside, the word runs towards the nose on the right and towards the tail on the left.
    cursor = -text_width / 2
    for letter in WORD:
        width, lines = LETTERS[letter]
        for line in lines:
            placed = [(TEXT_MIDDLE + side * (cursor + u * TEXT_HIGH), TEXT_FOOT + v * TEXT_HIGH) for u, v in line]
            for start, end in zip(placed, placed[1:]):
                marking([tuple(p) for p in stroke(start, end, 0.042)], GREEN, side)
        cursor += (width + TEXT_GAP) * TEXT_HIGH
    star_of_life((1.32, 0.05), 0.15, side)
    # One row of squares along the belly, doubling into chequers towards the tail.
    for k in range(5):
        front = 1.42 - k * 0.38
        marking([(front, -0.32), (front - 0.19, -0.32), (front - 0.19, -0.15), (front, -0.15)], GREEN, side)
    for k in range(5):
        front, foot = -0.68 - k * 0.19, -0.29 if k % 2 else -0.11
        marking([(front, foot), (front - 0.19, foot), (front - 0.19, foot + 0.18), (front, foot + 0.18)], GREEN, side)
    star_of_life((-5.86, 0.66), 0.12, side, x=0.14)

# --------------------------------------------------------------------------------------------------
# The rest of the airframe
# --------------------------------------------------------------------------------------------------
hull = Part("hull")

# The engines' housing on the roof, the gearbox's cowl on that, and the exhausts out of its back.
hull.loft(
    [
        (0.3, 0.0, 1.31, 0.28, 0.05),
        (-0.1, 0.0, 1.4, 0.48, 0.27),
        (-1.25, 0.0, 1.43, 0.58, 0.32),
        (-2.2, 0.0, 1.3, 0.52, 0.3),
        (-2.95, 0.0, 1.04, 0.3, 0.22),
        (-3.3, 0.0, 0.86, 0.14, 0.08),
    ],
    LIVERY,
    segments=10,
    power=2.8,
)
hull.loft([(0.02, 0.0, 1.66, 0.14, 0.04), (-0.38, 0.0, 1.72, 0.3, 0.18), (-1.12, 0.0, 1.72, 0.3, 0.17), (-1.5, 0.0, 1.62, 0.16, 0.05)], LIVERY, segments=8, power=3.0)
for side in (-1, 1):
    hull.cylinder((side * 0.36, -2.5, 1.36), 0.17, 0.5, DARK, axis="Y", segments=8, radius2=0.14)  # exhaust
    hull.box((side * 0.57, -1.0, 1.45), (0.04, 0.3, 0.12), DARK)  # intake grille
hull.cylinder((0.0, ROTOR_HEAD.y, 1.92), 0.11, 0.36, DARK, segments=6)  # mast

# The boom, the tailplane across it with its end plates, and the fin round the fan.
hull.loft([(-3.05, 0.0, 0.74, 0.26, 0.27), (-4.0, 0.0, 0.72, 0.19, 0.21), (-5.85, 0.0, 0.7, 0.14, 0.16)], LIVERY, segments=8)
hull.box((0.0, -5.2, 0.68), (2.6, 0.46, 0.06), LIVERY)
for side in (-1, 1):
    hull.prism([(-4.95, 0.86), (-5.55, 1.02), (-5.66, 0.28), (-5.25, 0.44)], side * 1.3 - 0.025, side * 1.3 + 0.025, LIVERY)
SEGMENTS = 14
around = [(math.cos(2 * math.pi * k / SEGMENTS), math.sin(2 * math.pi * k / SEGMENTS)) for k in range(SEGMENTS)]
hull.ring([(FAN_HUB.y + 0.65 * c, FAN_HUB.z + 0.64 * s) for c, s in around], [(FAN_HUB.y + 0.45 * c, FAN_HUB.z + 0.45 * s) for c, s in around], -0.135, 0.135, LIVERY)
hull.prism([(-5.64, 0.9), (-6.07, 1.04), (-6.0, 0.66), (-6.07, 0.28), (-5.64, 0.5)], -0.14, 0.14, LIVERY)
hull.prism([(-6.05, 1.05), (-6.78, 2.3), (-7.5, 2.32), (-7.04, 0.98), (-6.86, 1.15), (-6.55, 1.22), (-6.24, 1.15)], -0.07, 0.07, LIVERY)
hull.prism([(-6.0, 0.22), (-6.3, -0.38), (-6.95, -0.43), (-7.05, 0.18), (-6.8, 0.1), (-6.55, 0.06), (-6.3, 0.1)], -0.06, 0.06, RED)
hull.box((0.0, -7.1, 2.36), (0.07, 0.12, 0.08), RED)  # beacon

# The skids, their cross tubes, and a step along each.
for side in (-1, 1):
    x = side * 1.0
    hull.tube((x, 1.3, GROUND + 0.055), (x, -1.65, GROUND + 0.055), 0.055, STEEL)
    hull.tube((x, 1.3, GROUND + 0.055), (x, 1.72, GROUND + 0.22), 0.055, STEEL)
    hull.box((x, 0.1, GROUND + 0.15), (0.17, 1.7, 0.04), STEEL)
    for y in (0.95, -1.05):
        hull.tube((side * 0.4, y, -0.53), (x, y, GROUND + 0.07), 0.05, DARK)
hull.cylinder(LAMP, 0.12, 0.16, DARK, axis="Y", segments=8)
hull.cylinder(LAMP + Vector((0.0, 0.085, 0.0)), 0.09, 0.02, WHITE, axis="Y", segments=8)

# Inside: the floor, the panel and a seat under each head.
hull.box((0.0, 0.1, -0.48), (1.1, 2.8, 0.04), LINING)
hull.box((0.0, 1.72, 0.1), (0.85, 0.16, 0.34), DARK)
for x, y, _ in SEAT_HEADS:
    hull.box((x, y + 0.03, -0.14), (0.44, 0.46, 0.1), DARK)
    hull.box((x, y - 0.25, 0.22), (0.44, 0.09, 0.66), DARK)

hull_obj = hull.build()
cabin.parent = hull_obj
cabin_obj = cabin.build()

# --------------------------------------------------------------------------------------------------
# Main rotor: four long blades that cone a little, under a yellow cap. Tail fan: ten blades in the fin.
# --------------------------------------------------------------------------------------------------
rotor = Part("rotor", location=ROTOR_HEAD)
rotor.cylinder((0, 0, -0.03), 0.3, 0.12, DARK, segments=8)
rotor.cylinder((0, 0, 0.1), 0.33, 0.16, LIVERY, segments=8, radius2=0.14)
for k in range(4):
    turn = about_z(math.pi / 4 + k * math.pi / 2) @ about_y(-0.03)
    for start, end, width, thick, mat in ((0.2, 0.85, 0.16, 0.08, DARK), (0.8, ROTOR_RADIUS - 0.3, 0.29, 0.035, DARK), (ROTOR_RADIUS - 0.3, ROTOR_RADIUS, 0.29, 0.035, STEEL)):
        rotor.box(turn @ Vector(((start + end) / 2, 0, 0)), (end - start, width, thick), mat, rot=turn)
rotor_obj = rotor.build()

fan = Part("tail_rotor", location=FAN_HUB)
fan.cylinder((0, 0, 0), 0.12, 0.24, LIVERY, axis="X", segments=8)
for k in range(10):
    turn = about_x(2 * math.pi * k / 10)
    fan.box(turn @ Vector((0.0, 0.0, 0.27)), (0.03, 0.06, 0.32), DARK, rot=turn)
fan_obj = fan.build()

for index, at in enumerate(SEAT_HEADS):
    marker = bpy.data.objects.new(f"seat_{index}", None)
    marker.location = at
    scene.collection.objects.link(marker)

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, cabin_obj, rotor_obj, fan_obj]
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
    # As the game draws it: fronts of faces only, and glass to see through.
    scene.display.shading.show_backface_culling = True
    MATERIALS[GLASS].diffuse_color[3] = 0.35
    scene.render.resolution_x, scene.render.resolution_y = 1280, 720
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    middle = Vector((0.0, -1.9, 0.5))
    views = {
        "side": ("ORTHO", Vector((-18, -1.9, 0.5)), middle, 12.8),
        "right": ("ORTHO", Vector((18, -1.9, 0.5)), middle, 12.8),
        "door": ("ORTHO", Vector((-8, 0.3, 0.2)), Vector((0, 0.3, 0.2)), 5.2),
        "top": ("ORTHO", Vector((0, -1.9, 16)), middle, 13.5),
        "front": ("ORTHO", Vector((0, 14, 0.5)), Vector((0, 0, 0.5)), 6.5),
        "three": ("PERSP", Vector((-8.5, 8.0, 3.4)), Vector((0, -1.2, 0.4)), 0),
        "rear": ("PERSP", Vector((7.5, -12.5, 4.0)), Vector((0, -2.5, 0.5)), 0),
    }
    for name, (kind, at, target, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = at
        cam.rotation_euler = (target - at).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)


if PREVIEW:
    preview(PREVIEW)
