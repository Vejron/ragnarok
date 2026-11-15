"""
Builds the Titan (a giant robot about fifty metres high) and exports it to client/public/models/titan.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_titan.py -- \
        [--out client/public/models/titan.glb] [--preview DIR]

Everything is procedural and flat-shaded: slabs of grey armour over a dark frame, with a glowing core in
the chest and a glowing visor.

Conventions
  * Blender: Z up, the chest faces +Y, the Titan's right hand side is +X. The glTF exporter turns that
    into Y up, chest along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider, its torso (a 12 x 16 x 8 m box, TITAN_HALF_EXTENTS).
  * Nodes the game looks up by name. None is the child of another limb: the game places each by IK.
      `torso`               chest, shoulders, collar and hips, fixed to the hull, with the missile launcher
                            on the left pauldron (TITAN_LAUNCHER_*, TITAN_TUBE_AT)
      `head`                the helmet, origin at the neck it turns on (TITAN_HEAD_PIVOT), the top of the collar
      `upper_L/R`           upper arm, origin at the shoulder (TITAN_SHOULDER), hanging down UPPER metres
      `fore_L/R`            forearm, origin at the elbow, hanging down FORE metres to the fist's middle
      `fist_L/R`            the hand's block, origin at its middle (a ball of TITAN_FIST_RADIUS in the physics)
      `open_L/R`            child of the fist: its fingers held out
      `closed_L/R`          child of the fist: its fingers curled in; the game shows one of the two
      `gun_R`               child of the right fist: the 40 mm autocannon it holds, its barrel along the
                            forearm out to GUN metres past the fist's middle (TITAN_GUN_LENGTH)
      `thigh_L/R`           origin at the hip (TITAN_FOOT_HALF sideways, the bottom of the hull), hanging THIGH metres
      `shin_L/R`            origin at the knee, hanging SHIN metres to the ankle
      `foot_L/R`            origin at the ankle; its sole is PAD below
    Every limb hangs straight down in the rest pose.
  * Materials: `paint` is tinted per Titan at runtime; `dark`, `metal`, `steel` keep their colours and
    `lamp` is unlit (the core, the visor, the vents).
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
GUN = 7.0  # from the right fist's middle to the gun's muzzle, along the forearm: TITAN_GUN_LENGTH
# The launcher's box on the left pauldron, its centre and size, and its four tubes' mouths (x, y) on its top.
LAUNCHER = Vector((-9.2, 0.0, 11.45))  # game TITAN_LAUNCHER_AT [-9.2, 11.45, 0.0]
LAUNCHER_SIZE = (3.8, 4.6, 2.5)  # twice TITAN_LAUNCHER_HALF, as (x, y, z) here
TUBES = [(-10.1, 1.1), (-8.3, 1.1), (-10.1, -1.1), (-8.3, -1.1)]  # TITAN_TUBE_AT's x and -z

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene

MATERIALS[:] = [
    material("paint", (0.5, 0.53, 0.56)),
    material("dark", (0.11, 0.12, 0.14)),
    material("metal", (0.27, 0.29, 0.32)),
    material("steel", (0.62, 0.64, 0.66)),
    material("lamp", (1.0, 0.48, 0.1)),
]
PAINT, DARK, METAL, STEEL, LAMP = range(5)


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


# --------------------------------------------------------------------------------------------------
# Torso: a broad chest that narrows to the waist, the core glowing in the middle of it, pauldrons over
# the shoulders, a collar for the head between them and a reactor on the back
# --------------------------------------------------------------------------------------------------
torso = Part("torso")
torso.taper((0.0, 0.0, 3.0), (2 * HX + 1.0, 2 * HY + 0.6), (2 * HX - 2.0, 2 * HY - 0.6), 10.0, PAINT)  # chest
torso.taper((0.0, 0.0, -4.5), (2 * HX - 2.6, 2 * HY - 1.4), (2 * HX - 4.4, 2 * HY - 2.4), 5.0, DARK)  # waist
torso.taper((0.0, 0.0, -7.6), (2 * HX - 1.0, 2 * HY - 0.6), (2 * HX + 0.6, 2 * HY - 0.2), 2.6, PAINT)  # hips
torso.box((0.0, HY - 0.2, -8.2), (3.6, 1.4, 3.2), METAL)  # codpiece
# Chest plates either side of the core, raked back, and the core itself in a dark ring.
for side in (-1, 1):
    torso.box((side * 3.6, HY + 0.35, 4.4), (4.6, 0.9, 5.4), PAINT, rx=-0.12)
    torso.box((side * 3.4, HY + 0.2, 0.2), (3.6, 0.7, 2.4), METAL, rx=0.2)
    torso.box((side * 5.9, 0.0, 1.5), (0.9, 5.6, 6.0), METAL)  # flank
    for k in range(3):
        torso.box((side * 6.4, 0.0, 3.4 - k * 1.5), (0.3, 4.2, 0.5), LAMP)  # side vents
torso.cylinder((0.0, HY + 0.3, 3.2), 2.3, 0.9, DARK, axis="Y", segments=12)
torso.cylinder((0.0, HY + 0.55, 3.2), 1.6, 0.7, LAMP, axis="Y", segments=12)
torso.cylinder((0.0, HY + 0.75, 3.2), 0.7, 0.6, STEEL, axis="Y", segments=8)
# Collar: the head (below) turns on its top.
torso.box((0.0, 0.0, 8.6), (7.0, 6.0, 1.6), DARK)
# Pauldrons: great slabs over each shoulder joint.
for side in (-1, 1):
    torso.cylinder((side * 8.0, 0.0, 6.0), 2.6, 3.6, DARK, axis="X", segments=10)  # the joint's housing
    torso.taper((side * 9.2, 0.0, 8.6), (5.4, 7.6), (7.0, 8.8), 2.4, PAINT)
    torso.box((side * 11.6, 0.0, 7.0), (1.2, 8.0, 4.4), PAINT)
    torso.box((side * 9.2, 0.0, 9.95), (3.4, 5.0, 0.5), METAL)
# The missile launcher on the left pauldron: an armoured box with four tube hatches in its top, and a
# cheek plate down its outer side.
top = LAUNCHER.z + LAUNCHER_SIZE[2] / 2
torso.box(LAUNCHER, LAUNCHER_SIZE, METAL)
torso.box((LAUNCHER.x - LAUNCHER_SIZE[0] / 2 - 0.2, 0.0, LAUNCHER.z - 0.2), (0.4, LAUNCHER_SIZE[1] + 0.4, LAUNCHER_SIZE[2] + 0.4), PAINT)
for x, y in TUBES:
    torso.cylinder((x, y, top + 0.05), 0.75, 0.3, DARK, segments=10)  # the hatch's rim
    torso.cylinder((x, y, top + 0.2), 0.55, 0.15, PAINT, segments=10)  # the hatch
    torso.box((x, y, top + 0.3), (0.3, 1.0, 0.1), LAMP)  # its warning stripe
# The back: a reactor housing with two stacks.
torso.box((0.0, -HY - 1.0, 3.6), (7.6, 2.4, 8.0), METAL)
torso.box((0.0, -HY - 2.3, 3.6), (5.0, 0.5, 5.6), DARK)
for side in (-1, 1):
    torso.cylinder((side * 2.6, -HY - 1.4, 9.4), 0.9, 5.0, DARK, segments=8)
    torso.cylinder((side * 2.6, -HY - 1.4, 12.0), 0.6, 0.3, LAMP, segments=8)
    torso.cylinder((side * FOOT_HALF, 0.0, -HZ), 2.4, 3.4, DARK, axis="X", segments=10)  # hip joints
torso_obj = torso.build()
objects = [torso_obj]

# --------------------------------------------------------------------------------------------------
# Head: a low helmet with a visor slit, its chin behind a guard. Its origin is the neck it turns on
# --------------------------------------------------------------------------------------------------
head = Part("head", location=NECK)
head.taper((0.0, 0.4, 1.9), (3.6, 4.4), (4.6, 5.0), 4.2, PAINT)
head.box((0.0, 2.75, 2.2), (3.6, 0.5, 0.9), LAMP)  # visor
head.box((0.0, 2.8, 3.1), (4.2, 0.9, 0.7), DARK, rx=0.35)  # brow
head.box((0.0, 2.7, 0.8), (3.0, 0.8, 1.6), METAL)  # chin guard
head.box((0.0, -0.2, 4.3), (1.2, 3.6, 0.9), METAL)  # crest
for side in (-1, 1):
    head.box((side * 2.5, 0.2, 1.9), (0.7, 3.0, 2.6), DARK)  # ear plates
    head.cylinder((side * 2.2, -1.6, 5.2), 0.12, 3.4, METAL, segments=5)  # antennae
objects.append(head.build())

# --------------------------------------------------------------------------------------------------
# Arms: an upper arm in a sleeve, a heavier forearm with a guard along its back, and a fist
# --------------------------------------------------------------------------------------------------
for side, label in ((-1, "L"), (1, "R")):
    shoulder = Vector((side * SHOULDER.x, SHOULDER.y, SHOULDER.z))
    upper = Part(f"upper_{label}", location=shoulder)
    upper.cylinder((0, 0, 0), 2.0, 3.4, METAL, axis="X", segments=10)
    upper.taper((0.0, 0.0, -UPPER / 2), (3.6, 3.8), (3.0, 3.2), UPPER - 3.0, DARK)
    upper.taper((side * 0.9, 0.0, -UPPER / 2 + 0.6), (2.6, 4.4), (2.2, 3.8), UPPER - 5.0, PAINT)  # sleeve plate
    upper.cylinder((0.0, 1.7, -UPPER / 2), 0.35, UPPER - 4.0, STEEL, segments=6)  # piston
    objects.append(upper.build())

    fore = Part(f"fore_{label}", location=shoulder + Vector((0, 0, -UPPER)))
    fore.cylinder((0, 0, 0), 2.2, 4.0, METAL, axis="X", segments=10)  # elbow
    fore.taper((0.0, 0.0, -FORE / 2 + 0.6), (4.0, 4.4), (4.8, 5.0), FORE - 4.4, PAINT)
    fore.box((side * 2.5, 0.0, -FORE / 2 + 0.4), (0.8, 3.2, FORE - 5.4), METAL)  # guard
    fore.box((0.0, -2.6, -FORE / 2 + 1.4), (2.4, 0.8, 5.0), DARK)
    for k in range(3):
        fore.box((0.0, 2.6, -3.4 - k * 1.5), (2.8, 0.4, 0.6), LAMP)  # vents down the front
    fore.cylinder((0.0, 0.0, -FORE + FIST + 0.4), 1.8, 1.6, DARK, segments=10)  # wrist
    objects.append(fore.build())

    # The fist: a block as wide as the physics' ball, knuckles down, thumb to the front.
    fist = Part(f"fist_{label}", location=shoulder + Vector((0, 0, -UPPER - FORE)))
    fist.box((0.0, 0.0, 0.4), (4.4, 4.0, 3.2), PAINT)  # the back of the hand
    fist.box((side * 2.3, 0.0, 0.6), (0.5, 3.2, 2.4), METAL)
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
        # The 40 mm in the right fist: its barrel along the forearm (down, at rest) through the fingers to
        # the muzzle brake, on the line its rounds leave by, a receiver on the back of the hand, and a drum
        # of rounds on its outer side.
        gun = Part("gun_R", parent=fist_obj)
        gun.box((0.0, -0.4, 0.2), (2.4, 2.0, 4.2), DARK)  # receiver
        gun.cylinder((0.0, 0.0, -GUN / 2 + 0.2), 1.0, GUN - 2.6, METAL, segments=10)  # barrel jacket
        gun.cylinder((0.0, 0.0, -GUN / 2 - 0.8), 0.6, GUN - 1.2, STEEL, segments=8)  # barrel
        gun.cylinder((0.0, 0.0, -GUN + 0.5), 1.05, 1.0, DARK, segments=8)  # muzzle brake
        gun.cylinder((side * 3.0, -0.2, 0.6), 1.6, 1.4, METAL, axis="X", segments=12)  # drum
        gun.cylinder((side * 3.75, -0.2, 0.6), 0.5, 0.1, LAMP, axis="X", segments=8)  # its round counter
        objects.append(gun.build())

# --------------------------------------------------------------------------------------------------
# Legs: a heavy thigh, a knee cap, a shin that flares to the ankle and a broad foot
# --------------------------------------------------------------------------------------------------
for side, label in ((-1, "L"), (1, "R")):
    hip = Vector((side * FOOT_HALF, 0.0, -HZ))
    thigh = Part(f"thigh_{label}", location=hip)
    thigh.taper((0.0, 0.0, -THIGH / 2), (4.6, 5.2), (3.8, 4.4), THIGH - 3.0, DARK)
    thigh.taper((side * 0.9, 0.5, -THIGH / 2 + 0.8), (3.4, 5.6), (3.0, 4.8), THIGH - 6.0, PAINT)  # outer plate
    thigh.box((0.0, 2.7, -THIGH / 2 + 1.0), (2.6, 0.7, 5.6), METAL)
    thigh.cylinder((0.0, -2.3, -THIGH / 2), 0.4, THIGH - 5.0, STEEL, segments=6)  # piston behind
    objects.append(thigh.build())

    shin = Part(f"shin_{label}", location=hip + Vector((0, 0, -THIGH)))
    shin.cylinder((0, 0, 0), 2.5, 4.6, METAL, axis="X", segments=10)  # knee
    shin.box((0.0, 2.4, 0.2), (3.6, 1.2, 3.8), PAINT, rx=-0.15)  # knee cap
    shin.taper((0.0, 0.0, -SHIN / 2), (3.8, 4.4), (4.8, 5.8), SHIN - 4.0, PAINT)
    shin.box((0.0, 2.9, -SHIN / 2 - 0.8), (2.8, 0.8, 6.0), METAL)  # shin guard
    shin.box((0.0, -2.9, -SHIN / 2 - 0.4), (2.4, 0.8, 5.0), DARK)
    for k in range(2):
        shin.box((side * 2.5, 0.0, -4.4 - k * 1.6), (0.4, 3.0, 0.7), LAMP)  # calf vents
    objects.append(shin.build())

    foot = Part(f"foot_{label}", location=hip + Vector((0, 0, -THIGH - SHIN)))
    foot.cylinder((0, 0, 0), 1.9, 4.8, METAL, axis="X", segments=10)  # ankle
    foot.box((0.0, 0.6, -PAD / 2 - 0.1), (5.2, 6.8, PAD - 0.2), DARK)  # sole
    foot.taper((0.0, 2.6, -PAD / 2 + 0.5), (4.4, 3.4), (5.2, 4.4), 1.6, PAINT)  # toe cap
    foot.box((0.0, -2.9, -PAD / 2 + 0.3), (4.0, 1.6, 1.4), METAL)  # heel
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
