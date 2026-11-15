"""
Builds the Titan (a giant robot about fifty metres high) and exports it to client/public/models/titan.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_titan.py -- \
        [--out client/public/models/titan.glb] [--preview DIR]

Everything is procedural and flat-shaded: a stocky, angular machine in two colours, slabs of armour
over a darker second colour and a dark frame, with low blocks over its shoulders, a small helmet sunk
between them, heavy forearms behind shields and broad feet.

Conventions
  * Blender: Z up, the chest faces +Y, the Titan's right hand side is +X. The glTF exporter turns that
    into Y up, chest along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider, its torso (a 12 x 16 x 8 m box, TITAN_HALF_EXTENTS).
  * Nodes the game looks up by name. None is the child of another limb: the game places each by IK.
      `torso`               chest, shoulder blocks, collar and hips, fixed to the hull, with the four missile
                            hatches set in the top of the left shoulder block (TITAN_TUBE_AT)
      `head`                the helmet, origin at the neck it turns on (TITAN_HEAD_PIVOT), the top of the collar
      `upper_L/R`           upper arm, origin at the shoulder (TITAN_SHOULDER), hanging down UPPER metres
      `fore_L/R`            forearm, origin at the elbow, hanging down FORE metres to the fist's middle
      `fist_L/R`            the hand's block, origin at its middle (a ball of TITAN_FIST_RADIUS in the physics)
      `open_L/R`            child of the fist: its fingers held out
      `closed_L/R`          child of the fist: its fingers curled in; the game shows one of the two
      `gun_R`               child of the right fist: the rotary cannon's housing it holds, and its drum
      `barrels_R`           child of the right fist: the cannon's three barrels round the forearm's line, out
                            to GUN metres past the fist's middle (TITAN_GUN_LENGTH); the game turns them
                            about that line (their Z here), a third of a turn a round
      `thigh_L/R`           origin at the hip (TITAN_FOOT_HALF sideways, the bottom of the hull), hanging THIGH metres
      `shin_L/R`            origin at the knee, hanging SHIN metres to the ankle
      `foot_L/R`            origin at the ankle; its sole is PAD below
    Every limb hangs straight down in the rest pose.
  * Materials: `paint` (the armour) and `trim` (the second colour under it) are tinted per Titan at
    runtime; `dark`, `metal`, `steel` keep their colours, and `lamp` (the chest's slot, the vents, the
    thrusters) and `visor` are unlit.
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Vector

sys.dont_write_bytecode = True
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

import common
from common import MATERIALS, arg, export, material

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/titan.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with TITAN_* in crates/shared/src/vehicles/titan.rs and
# render/titan.ts.
# --------------------------------------------------------------------------------------------------
HX, HY, HZ = 6.0, 4.0, 8.0  # half extents of the torso box: sideways, front to back, up
FOOT_HALF = 5.0
SHOULDER = Vector((8.5, 0.0, 6.0))  # game [8.5, 6.0, 0.0]
NECK = Vector((0.0, 0.0, 9.4))  # game TITAN_HEAD_PIVOT [0.0, 9.4, 0.0]
UPPER, FORE = 12.0, 12.0  # shoulder to elbow, elbow to the fist's middle: TITAN_REACH in all
FIST = 2.5  # the fist's radius in the physics
THIGH, SHIN = 13.5, 13.5  # longer than the legs' reach, so that it stands with its knees bent
PAD = 1.5  # the foot's thickness: ankle origin to sole
GUN = 8.0  # from the right fist's middle to the gun's muzzles, along the forearm: TITAN_GUN_LENGTH
BARRELS, BARREL_OFFSET = 3, 0.75  # TITAN_GUN_BARRELS, TITAN_GUN_BARREL_OFFSET: round the forearm's line
# The shoulder blocks' flat tops, and the four missile hatches' mouths (x, y) in the left one's.
BLOCK_TOP = 11.2
TUBES = [(-9.6, 1.0), (-7.6, 1.0), (-9.6, -1.0), (-7.6, -1.0)]  # TITAN_TUBE_AT's x and -z
TUBE_MOUTH = BLOCK_TOP + 0.1  # TITAN_TUBE_AT's y

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

MATERIALS[:] = [
    material("paint", (0.86, 0.6, 0.1)),
    material("trim", (0.09, 0.19, 0.42)),
    material("dark", (0.11, 0.12, 0.14)),
    material("metal", (0.27, 0.29, 0.32)),
    material("steel", (0.62, 0.64, 0.66)),
    material("lamp", (1.0, 0.48, 0.1)),
    material("visor", (0.95, 0.12, 0.1)),
]
PAINT, TRIM, DARK, METAL, STEEL, LAMP, VISOR = range(7)


# The big plates are cut into a grid this fine, metres, so a shell's dent (scars.wgsl, about a metre
# across on a Titan) has vertices to press in; flat faces stay flat. Only faces of at least PLATE_AREA
# square metres are cut: the small parts and the drums' sides are left as they are, to keep it cheap.
DENT_GRID = 1.2
PLATE_AREA = 6.0


class Part(common.Part):
    def build(self):
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts, dist=1e-4)
        plates = {face for face in self.bm.faces if face.calc_area() >= PLATE_AREA}
        for _ in range(6):
            long = {edge for face in plates if face.is_valid for edge in face.edges if edge.calc_length() > DENT_GRID}
            if not long:
                break
            made = bmesh.ops.subdivide_edges(self.bm, edges=list(long), cuts=1, use_grid_fill=True)
            # The faces the cut plates became are plates still.
            plates = {face for face in self.bm.faces if face.calc_area() >= DENT_GRID * DENT_GRID * 0.5 and any(e.calc_length() > DENT_GRID for e in face.edges)}
        bmesh.ops.triangulate(self.bm, faces=[face for face in self.bm.faces if len(face.verts) > 4])
        return super().build()

    def taper(self, centre, top, bottom, height, mat):
        """A box that narrows (or widens) from `top` to `bottom`, each an (x, y) size, `height` tall along Z."""
        cx, cy, cz = centre
        corners = lambda size, z: [(cx + sx * size[0] / 2, cy + sy * size[1] / 2, cz + z) for sx, sy in ((-1, -1), (1, -1), (1, 1), (-1, 1))]
        t = [self.bm.verts.new(p) for p in corners(top, height / 2)]
        b = [self.bm.verts.new(p) for p in corners(bottom, -height / 2)]
        faces = [self.bm.faces.new(t), self.bm.faces.new(b[::-1])]
        for i in range(4):
            j = (i + 1) % 4
            faces.append(self.bm.faces.new((t[i], b[i], b[j], t[j])))
        for face in faces:
            face.material_index = mat

    def hull(self, points, mat, side=1):
        """The angular plate with these corners (their convex hull), its flat faces left whole. `side`
        of -1 mirrors it to the left hand side."""
        verts = [self.bm.verts.new((side * x, y, z)) for x, y, z in points]
        made = bmesh.ops.convex_hull(self.bm, input=verts)["geom"]
        faces = [g for g in made if isinstance(g, bmesh.types.BMFace)]
        for face in faces:
            face.material_index = mat
        bmesh.ops.dissolve_limit(
            self.bm,
            angle_limit=0.01,
            verts=list({v for face in faces for v in face.verts}),
            edges=list({e for face in faces for e in face.edges}),
        )


def both(points):
    """`points`, each given for +X, with their mirror images in -X."""
    return [(sx * x, y, z) for x, y, z in points for sx in (-1, 1)]


# --------------------------------------------------------------------------------------------------
# Torso: a chest like a wedge, broad at the shoulders and narrow at the waist, under two raked plates
# that meet in a prow; a ribbed waist, a pelvis, a low block over each shoulder with the collar for
# the head sunk between them, and a reactor on the back
# --------------------------------------------------------------------------------------------------
torso = Part("torso")
torso.taper((0.0, 0.0, 4.2), (13.6, 8.4), (9.0, 6.4), 7.6, TRIM)  # chest
torso.taper((0.0, 0.0, -3.0), (8.4, 6.0), (7.4, 5.2), 6.8, DARK)  # waist
torso.taper((0.0, 0.0, -7.4), (10.4, 6.6), (11.6, 7.0), 2.4, TRIM)  # pelvis
torso.hull(both([(1.9, 3.3, -6.4), (1.9, 4.5, -6.8), (1.2, 4.5, -9.8), (1.2, 3.3, -9.8)]), PAINT)  # codpiece
# The slot between the chest plates, glowing with the reactor behind it.
torso.box((0.0, 4.5, 5.3), (1.0, 0.8, 4.4), DARK)
torso.box((0.0, 4.75, 5.3), (0.45, 0.5, 3.6), LAMP)
for k in range(3):
    z = -0.8 - k * 2.0
    torso.box((0.0, 2.9, z), (6.8 - k * 0.4, 0.7, 1.3), METAL)  # the waist's ribs
    torso.box((0.0, 3.2, z), (3.2 - k * 0.4, 0.4, 0.5), PAINT)
for side in (-1, 1):
    # A chest plate: furthest forward at the middle, its lower edge rising to the armpit.
    torso.hull(
        [(0.35, 3.4, 3.0), (0.35, 4.9, 3.0), (0.35, 3.4, 7.4), (0.35, 5.3, 7.2), (6.3, 3.4, 4.4), (6.3, 4.0, 4.4), (6.9, 3.4, 7.8), (6.9, 4.4, 7.8)],
        PAINT,
        side,
    )
    torso.box((side * 3.0, 4.15, 2.3), (3.4, 0.5, 0.7), PAINT, rx=0.2)  # a bar under it
    torso.box((side * 5.6, 0.0, 2.6), (1.4, 5.2, 4.0), METAL)  # flank
    for k in range(2):
        torso.box((side * 6.3, 0.0, 3.4 - k * 1.3), (0.25, 3.8, 0.45), LAMP)  # side vents
    torso.box((side * 6.0, 0.0, -7.2), (0.8, 5.6, 2.6), PAINT)  # hip plate
    # The plate from the collar up to the shoulder block.
    torso.hull(
        [(3.0, -3.0, 8.0), (3.0, 3.4, 8.0), (7.2, -3.4, 8.0), (7.2, 3.8, 8.0), (3.0, -2.6, 9.0), (3.0, 3.0, 9.0), (7.2, -3.0, 9.7), (7.2, 3.4, 9.7)],
        PAINT,
        side,
    )
    # The shoulder: the joint's housing, and over it the block, flat on top, sloping down to its outer
    # edge, with a skirt hanging from that edge outside the upper arm.
    torso.cylinder((side * 8.0, 0.0, 6.0), 2.6, 3.6, DARK, axis="X", segments=8)
    torso.hull(
        [
            (6.4, -3.6, 8.7), (6.4, 4.0, 8.7), (11.0, -3.8, 8.7), (11.0, 4.2, 8.7),
            (6.6, -3.0, BLOCK_TOP), (6.6, 3.2, BLOCK_TOP), (10.5, -3.0, BLOCK_TOP), (10.5, 3.2, BLOCK_TOP),
            (12.0, -3.4, 10.0), (12.0, 3.8, 10.0), (12.0, -3.4, 8.7), (12.0, 3.8, 8.7),
        ],
        PAINT,
        side,
    )
    torso.box((side * 8.8, 3.6, 9.95), (3.0, 0.4, 1.4), TRIM, rx=0.38)  # a band on its front
    torso.hull(
        [(11.0, -3.4, 8.7), (11.0, 3.8, 8.7), (12.0, -3.4, 8.7), (12.0, 3.8, 8.7), (11.4, -2.6, 6.6), (11.4, 3.0, 6.6), (12.3, -2.6, 6.6), (12.3, 3.0, 6.6)],
        PAINT,
        side,
    )
    torso.cylinder((side * FOOT_HALF, 0.0, -HZ), 2.4, 3.4, DARK, axis="X", segments=8)  # hip joints
# Collar: the head (below) turns on its top.
torso.box((0.0, 0.0, 8.6), (6.0, 6.0, 1.6), DARK)
# The missiles: four hatches set in the top of the left shoulder block, each with a warning stripe.
for x, y in TUBES:
    torso.cylinder((x, y, BLOCK_TOP), 0.75, 0.16, DARK, segments=10)  # the hatch's rim
    torso.cylinder((x, y, BLOCK_TOP + 0.04), 0.55, 0.2, TRIM, segments=10)  # the hatch
    torso.box((x, y, BLOCK_TOP + 0.14), (0.3, 1.0, 0.06), LAMP)  # its warning stripe
# And two grilles in the top of the right one.
for k in range(2):
    torso.box((8.6, 1.0 - k * 2.0, BLOCK_TOP), (2.8, 0.7, 0.16), DARK)
# The back: a reactor pack with two short stacks.
torso.box((0.0, -HY - 0.9, 4.0), (8.6, 2.2, 6.4), METAL)
torso.box((0.0, -HY - 2.1, 4.0), (5.6, 0.4, 4.4), DARK)
for side in (-1, 1):
    torso.cylinder((side * 2.8, -HY - 1.3, 8.6), 0.9, 3.4, DARK, segments=8)
    torso.cylinder((side * 2.8, -HY - 1.3, 10.35), 0.6, 0.2, LAMP, segments=8)
torso_obj = torso.build()
objects = [torso_obj]

# --------------------------------------------------------------------------------------------------
# Head: a small helmet, its crown and fin in the armour's colour over a jaw in the second, with a
# visor slit under a swept brow. Its origin is the neck it turns on
# --------------------------------------------------------------------------------------------------
head = Part("head", location=NECK)
head.taper((0.0, 0.4, 1.25), (4.3, 4.9), (4.4, 5.0), 2.9, TRIM)  # jaw
head.taper((0.0, 0.4, 3.35), (3.4, 4.2), (4.5, 5.1), 1.3, PAINT)  # crown
head.box((0.0, 2.85, 2.2), (3.4, 0.6, 0.8), VISOR)
head.box((0.0, 2.8, 3.1), (4.4, 0.9, 0.7), PAINT, rx=0.35)  # brow
head.hull(both([(1.5, 2.6, 1.6), (1.5, 3.2, 1.5), (0.9, 3.2, 0.0), (0.9, 2.6, 0.0)]), METAL)  # chin guard
head.hull(both([(0.35, -2.0, 3.9), (0.35, 2.4, 3.9), (0.2, -2.2, 4.75), (0.2, 0.6, 4.75)]), PAINT)  # fin
for side in (-1, 1):
    head.box((side * 2.5, 0.2, 1.7), (0.7, 3.0, 2.6), TRIM)  # cheek guards
    head.box((side * 2.9, 0.2, 2.2), (0.2, 1.6, 0.8), PAINT)
objects.append(head.build())

# --------------------------------------------------------------------------------------------------
# Arms: a slim upper arm under a cap plate, a much heavier forearm behind a shield that stands up past
# the elbow, and a fist
# --------------------------------------------------------------------------------------------------
for side, label in ((-1, "L"), (1, "R")):
    shoulder = Vector((side * SHOULDER.x, SHOULDER.y, SHOULDER.z))
    upper = Part(f"upper_{label}", location=shoulder)
    upper.cylinder((0, 0, 0), 2.0, 3.4, METAL, axis="X", segments=8)
    upper.taper((0.0, 0.0, -UPPER / 2), (3.0, 3.2), (2.6, 2.8), UPPER - 3.0, DARK)
    upper.taper((side * 1.3, 0.0, -4.6), (2.0, 4.2), (1.6, 3.4), 5.0, PAINT)  # cap plate
    upper.box((0.0, 0.0, -8.6), (3.3, 3.5, 1.2), TRIM)  # band above the elbow
    upper.cylinder((0.0, 1.6, -UPPER / 2), 0.35, UPPER - 4.0, STEEL, segments=6)  # piston
    objects.append(upper.build())

    fore = Part(f"fore_{label}", location=shoulder + Vector((0, 0, -UPPER)))
    fore.cylinder((0, 0, 0), 2.2, 4.0, METAL, axis="X", segments=8)  # elbow
    fore.taper((0.0, 0.0, -5.6), (4.4, 4.8), (5.2, 5.4), 7.6, TRIM)
    # The shield on its outer side: a slab from past the elbow down to the wrist.
    fore.prism([(-2.2, 2.6), (2.2, 2.6), (3.1, -1.0), (2.9, -9.6), (-2.9, -9.6), (-3.1, -1.0)], side * 2.4, side * 3.4, PAINT)
    fore.box((side * 3.45, 0.0, -4.4), (0.2, 3.4, 1.4), TRIM)  # a band across it
    fore.box((0.0, -2.8, -5.4), (2.4, 0.8, 5.0), DARK)
    for k in range(2):
        fore.box((0.0, 2.75, -4.4 - k * 1.6), (2.8, 0.4, 0.6), LAMP)  # vents down the front
    fore.cylinder((0.0, 0.0, -FORE + FIST + 0.4), 1.8, 1.6, DARK, segments=10)  # wrist
    objects.append(fore.build())

    # The fist: a block as wide as the physics' ball, knuckles down, thumb to the front.
    fist = Part(f"fist_{label}", location=shoulder + Vector((0, 0, -UPPER - FORE)))
    fist.box((0.0, 0.0, 0.4), (4.4, 4.0, 3.2), TRIM)  # the back of the hand
    fist.box((side * 2.35, 0.0, 0.5), (0.5, 3.4, 2.6), PAINT)  # its guard
    fist.box((0.0, 0.0, 1.9), (3.6, 3.2, 0.6), DARK)
    fist_obj = fist.build()
    objects.append(fist_obj)

    opened = Part(f"open_{label}", parent=fist_obj)
    for k in range(4):
        x = -1.65 + k * 1.1
        opened.box((x, 0.5, -2.6), (0.9, 1.1, 3.0), DARK)  # fingers held straight down
        opened.box((x, 0.5, -1.25), (1.0, 1.3, 0.7), METAL)  # knuckles
    opened.box((-side * 2.6, 1.6, -0.6), (0.9, 1.1, 2.6), DARK, rx=-0.5)  # thumb out
    objects.append(opened.build())

    closed = Part(f"closed_{label}", parent=fist_obj)
    for k in range(4):
        x = -1.65 + k * 1.1
        closed.box((x, 0.2, -1.75), (0.9, 3.4, 1.1), DARK)  # fingers curled under
        closed.box((x, 1.7, -1.3), (1.0, 0.9, 1.5), METAL)  # knuckles to the front
    closed.box((-side * 0.4, 2.1, -0.4), (3.0, 0.9, 1.0), DARK)  # thumb across them
    objects.append(closed.build())

    if label == "R":
        # The rotary cannon in the right fist: a housing gripped in the hand, with the motor that turns
        # the barrels on the back of it and a drum of rounds on its outer side.
        gun = Part("gun_R", parent=fist_obj)
        gun.box((0.0, -0.4, 0.4), (2.6, 2.2, 4.4), DARK)  # housing
        gun.cylinder((0.0, -1.9, 0.6), 0.9, 2.4, METAL, segments=10)  # the barrels' motor
        gun.cylinder((side * 3.4, -0.2, 0.6), 2.0, 1.8, METAL, axis="X", segments=12)  # drum
        gun.cylinder((side * 3.4, -0.2, 0.6), 2.1, 0.7, PAINT, axis="X", segments=12)  # the band round it
        gun.cylinder((side * 4.35, -0.2, 0.6), 0.6, 0.1, LAMP, axis="X", segments=8)  # its round counter
        objects.append(gun.build())

        # Its three barrels round the forearm's line, the line its rounds leave by, held by two clamps,
        # each with a heavy collar at the muzzle. A node of its own, for the game to turn.
        barrels = Part("barrels_R", parent=fist_obj)
        barrels.cylinder((0.0, 0.0, -1.2), 1.35, 1.0, METAL, segments=12)  # rear clamp, at the knuckles
        barrels.cylinder((0.0, 0.0, -GUN + 1.6), 1.3, 0.7, METAL, segments=12)  # front clamp
        barrels.cylinder((0.0, 0.0, -GUN / 2 - 0.3), 0.35, GUN - 1.4, DARK, segments=8)  # the spindle
        for k in range(BARRELS):
            turn = 2 * math.pi * k / BARRELS
            x, y = BARREL_OFFSET * math.cos(turn), BARREL_OFFSET * math.sin(turn)
            barrels.cylinder((x, y, -GUN / 2 - 0.4), 0.42, GUN - 1.2, STEEL, segments=8)  # barrel
            barrels.cylinder((x, y, -GUN + 0.3), 0.52, 0.6, DARK, segments=8)  # muzzle collar
        objects.append(barrels.build())

# --------------------------------------------------------------------------------------------------
# Legs: a thigh behind a great plate that is widest at the hip, an angular knee cap, a shin behind a
# greave that flares to the ankle, and a broad foot
# --------------------------------------------------------------------------------------------------
for side, label in ((-1, "L"), (1, "R")):
    hip = Vector((side * FOOT_HALF, 0.0, -HZ))
    thigh = Part(f"thigh_{label}", location=hip)
    thigh.taper((0.0, 0.0, -THIGH / 2), (4.2, 4.8), (3.6, 4.2), THIGH - 3.0, DARK)
    # The plate over its front and outer side; the frame shows behind it and on the inside.
    thigh.hull(
        [(-2.0, -1.6, -1.2), (-2.0, 3.3, -1.2), (3.2, -2.2, -1.2), (3.2, 3.3, -1.2), (-1.5, -1.0, -10.4), (-1.5, 2.8, -10.4), (2.6, -1.6, -10.4), (2.6, 2.8, -10.4)],
        PAINT,
        side,
    )
    thigh.box((side * 0.6, 3.0, -5.6), (2.2, 0.4, 3.2), TRIM)  # a band down its front
    thigh.cylinder((0.0, -2.3, -THIGH / 2), 0.4, THIGH - 5.0, STEEL, segments=6)  # piston behind
    objects.append(thigh.build())

    shin = Part(f"shin_{label}", location=hip + Vector((0, 0, -THIGH)))
    shin.cylinder((0, 0, 0), 2.5, 4.6, DARK, axis="X", segments=8)  # knee
    shin.hull(both([(1.9, 2.0, 2.4), (1.5, 3.6, 1.2), (1.9, 3.4, -1.6), (1.9, 2.0, -1.6)]), PAINT)  # knee cap
    shin.taper((0.0, 0.0, -6.9), (4.0, 4.4), (5.6, 6.0), 9.4, TRIM)
    shin.hull(both([(1.6, 2.0, -2.0), (1.6, 3.1, -2.0), (3.1, 2.4, -11.9), (3.1, 4.2, -11.9)]), PAINT)  # greave
    shin.box((side * 2.7, 0.2, -8.8), (0.5, 3.6, 5.2), PAINT)  # its outer side
    shin.box((0.0, -2.9, -7.2), (2.4, 0.8, 5.0), DARK)
    for k in range(2):
        shin.box((0.0, -3.35, -5.8 - k * 1.6), (1.6, 0.2, 0.6), LAMP)  # calf vents
    objects.append(shin.build())

    foot = Part(f"foot_{label}", location=hip + Vector((0, 0, -THIGH - SHIN)))
    foot.cylinder((0, 0, 0), 1.9, 4.8, METAL, axis="X", segments=8)  # ankle
    foot.box((0.0, 0.9, -PAD / 2 - 0.1), (6.4, 8.2, PAD - 0.2), DARK)  # sole
    foot.hull(both([(3.3, 0.6, -0.25), (3.3, 5.1, -0.25), (2.6, 0.6, 1.5), (2.6, 3.0, 1.3)]), PAINT)  # toe block
    foot.box((0.0, -2.5, -0.1), (5.2, 2.2, 1.6), TRIM)  # heel
    # Thrusters: two bells behind the heel, their mouths just clear of the ground, glowing inside.
    for bell in (-1, 1):
        foot.cylinder((bell * 1.5, -4.5, -0.2), 1.1, 2.4, STEEL, axis="Z", segments=8)
        foot.cylinder((bell * 1.5, -4.5, -1.42), 0.8, 0.12, LAMP, axis="Z", segments=8)
    objects.append(foot.build())

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
print("tris:", sum(len(p.vertices) - 2 for o in objects for p in o.data.polygons))
print("wrote", OUT)



# --------------------------------------------------------------------------------------------------
# Preview renders (optional): the shared ones stand too close for something this size
# --------------------------------------------------------------------------------------------------
def preview(directory):
    os.makedirs(directory, exist_ok=True)
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "MATERIAL"
    scene.render.resolution_x, scene.render.resolution_y = 800, 900
    cam_data = bpy.data.cameras.new("cam")
    cam_data.clip_end = 500.0
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    middle = Vector((0, 0, -10.5))
    views = {
        "side": ("ORTHO", Vector((80, 0, 0)), 54.0),
        "front": ("ORTHO", Vector((0, 80, 0)), 54.0),
        "three": ("PERSP", Vector((48, 62, 8)), 0),
        "rear": ("PERSP", Vector((-44, -62, 14)), 0),
    }
    for name, (kind, offset, scale) in views.items():
        cam_data.type = kind
        if kind == "ORTHO":
            cam_data.ortho_scale = scale
        cam.location = middle + offset
        cam.rotation_euler = (-offset).to_track_quat("-Z", "Y").to_euler()
        scene.render.filepath = os.path.join(directory, f"{name}.png")
        bpy.ops.render.render(write_still=True)


if PREVIEW:
    preview(PREVIEW)
