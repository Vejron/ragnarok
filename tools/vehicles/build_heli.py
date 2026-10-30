"""
Builds the helicopter (a Mil Mi-24 "Hind" in silhouette) and exports it to client/public/models/heli.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_heli.py -- \
        [--out client/public/models/heli.glb] [--preview DIR]

Everything is procedural and flat-shaded, a few thousand triangles in all, like the tank.

Conventions
  * Blender: Z up, the nose points along +Y, the helicopter's right hand side is +X. The glTF exporter turns
    that into Y up, nose along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.0 x 1.6 x 4.8 m box). The wheels touch the ground 0.8 m
    below it. The tail boom, the wings and the rotor stick out of that box, as the toy helicopter's did.
  * Nodes the game looks up by name:
      `hull`        everything fixed to the airframe, wings, pods and landing gear included
      `rotor`       origin on the rotor head: the game spins it about Y
      `tail_rotor`  origin on the tail rotor's hub: the game spins it about X
      `turret`      the chin gun's mount, origin on its pivot (HELI_GUN_PIVOT): the game yaws it
      `gun`         child of `turret`, origin on the pivot too; the game pitches it and slides it back in recoil
  * Materials: `paint` is tinted per helicopter at runtime; `glass` is drawn translucent; `dark` (blades,
    tyres, intakes), `metal` (pods, barrels, fittings) and `steel` (hubs, rims) keep their colours.
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
from common import MATERIALS, about_x, about_y, about_z, arg, export

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/heli.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with HELI_* in crates/shared/src/vehicles/.
# --------------------------------------------------------------------------------------------------
GUN_PIVOT = Vector((0.0, 2.0, -0.6))  # game [0, -0.6, -2.0]: +Z in the game is backwards
GUN_LENGTH = 1.3  # pivot to muzzle
ROTOR_HEAD = Vector((0.0, -0.1, 1.3))
ROTOR_RADIUS = 3.4
ROTOR_BLADES = 5
TAIL_ROTOR_HUB = Vector((0.2, -4.6, 1.0))
TAIL_ROTOR_RADIUS = 0.55
GROUND = -0.8

# --------------------------------------------------------------------------------------------------
# Scene and materials
# --------------------------------------------------------------------------------------------------
bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def material(name, rgb, alpha=1.0):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, alpha)
    bsdf.inputs["Roughness"].default_value = 1.0
    bsdf.inputs["Alpha"].default_value = alpha
    m.diffuse_color = (*rgb, alpha)
    return m


MATERIALS[:] = [
    material("paint", (0.42, 0.48, 0.34)),
    material("dark", (0.1, 0.1, 0.1)),
    material("metal", (0.24, 0.26, 0.23)),
    material("steel", (0.55, 0.56, 0.55)),
    material("glass", (0.62, 0.84, 0.92), 0.5),
]
PAINT, DARK, METAL, STEEL, GLASS = range(5)


class Part(common.Part):

    def box(self, centre, size, mat, rx=0.0, rot=None):
        """A box. `rot` is a 4x4 rotation about its own centre; `rx` is the shorthand for one about X."""
        turn = rot if rot is not None else Matrix.Rotation(rx, 4, "X")
        m = Matrix.Translation(centre) @ turn @ Matrix.Diagonal((*size, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None, squash=(1.0, 1.0)):
        """A cylinder or frustum (`radius2` is the far end's, towards +axis). `squash` stretches the cross-section."""
        turn = {"Z": Matrix.Identity(4), "X": Matrix.Rotation(math.pi / 2, 4, "Y"), "Y": Matrix.Rotation(-math.pi / 2, 4, "X")}[axis]
        flat = Matrix.Diagonal((squash[0], squash[1], 1.0, 1.0))
        m = Matrix.Translation(centre) @ turn @ flat
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

    def dome(self, centre, scale, mat, floor=0.0, u=12, v=7):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)


# --------------------------------------------------------------------------------------------------
# Hull: the airframe. A fuselage lofted from the nose to the tail boom, the double cockpit, the engines
# on its back, the stub wings with their pods, the tail and the landing gear.
# --------------------------------------------------------------------------------------------------
hull = Part("hull")

# (y, centre x, centre z, radius x, radius z): the nose, the gunner's section, the deep cabin, then the boom.
hull.loft(
    [
        (2.55, 0.0, -0.1, 0.1, 0.1),
        (2.3, 0.0, -0.12, 0.36, 0.33),
        (1.8, 0.0, -0.08, 0.58, 0.5),
        (1.0, 0.0, 0.0, 0.72, 0.62),
        (0.0, 0.0, 0.05, 0.78, 0.68),
        (-1.0, 0.0, 0.08, 0.74, 0.66),
        (-1.8, 0.0, 0.18, 0.55, 0.5),
        (-2.4, 0.0, 0.3, 0.36, 0.34),
        (-3.4, 0.0, 0.42, 0.26, 0.26),
        (-4.4, 0.0, 0.5, 0.2, 0.2),
    ],
    PAINT,
    segments=12,
    power=2.4,
)

# Cockpits: the gunner low in the nose under a glass bubble, the pilot raised behind and above him, the two
# split by an armoured plate. Side windows and a door on the cabin.
hull.dome((0.0, 1.55, 0.05), (0.5, 0.95, 0.5), GLASS, u=12, v=6)
hull.box((0.0, 2.38, -0.02), (0.42, 0.06, 0.34), METAL)  # the flat armoured glass in front
hull.dome((0.0, 0.5, 0.42), (0.5, 0.85, 0.58), GLASS, u=12, v=6)
hull.box((0.0, 0.98, 0.5), (0.95, 0.1, 0.6), PAINT)  # armour plate between the two
hull.box((0.0, 0.38, 0.4), (0.9, 0.06, 0.1), PAINT)  # the pilot's windscreen frame
for side in (-1, 1):
    hull.box((side * 0.755, -0.5, 0.1), (0.04, 1.0, 0.34), GLASS)  # cabin window
    hull.box((side * 0.76, -0.5, -0.3), (0.04, 0.9, 0.5), METAL)  # door
    hull.box((side * 0.72, 1.3, 0.15), (0.05, 0.4, 0.2), METAL)  # cockpit step
# The nose sight under the gunner's bubble, an air-data probe on its side.
hull.box((0.0, 2.22, -0.38), (0.2, 0.26, 0.18), DARK)
hull.cylinder((0.32, 2.3, 0.0), 0.02, 0.5, METAL, axis="Y", segments=5)

# Engines: two nacelles on the cabin's back with dark intakes in front and exhaust cones behind, the main
# gearbox between them under the rotor mast, and the dust-filter hump over the intakes.
for side in (-1, 1):
    x = side * 0.43
    hull.cylinder((x, -0.1, 0.78), 0.27, 1.5, PAINT, axis="Y", segments=10)
    hull.cylinder((x, 0.7, 0.78), 0.22, 0.12, DARK, axis="Y", segments=10)
    hull.cylinder((x, -1.05, 0.78), 0.17, 0.5, METAL, axis="Y", segments=8, radius2=0.12)
    hull.box((x, 0.5, 0.97), (0.16, 0.5, 0.1), METAL)  # filter
hull.dome((0.0, -0.1, 0.75), (0.5, 0.9, 0.4), PAINT, u=12, v=5)
hull.cylinder((0.0, ROTOR_HEAD.y, 1.08), 0.13, 0.4, METAL, segments=8)  # mast
hull.box((0.0, 0.9, 0.7), (0.3, 0.3, 0.1), METAL)  # hatch on the roof

# Stub wings with anhedral: a pylon under each end carrying a rocket pod, and a missile on the tip rail.
WING_DROOP = 0.18  # how much lower the wing is, per metre out from the root


def wing_z(x):
    return -abs(x) * WING_DROOP


for side in (-1, 1):
    hull.box((side * 1.4, -0.35, wing_z(1.4)), (1.9, 0.62, 0.09), PAINT, rot=about_y(side * WING_DROOP))
    hull.box((side * 2.38, -0.35, wing_z(2.38) - 0.12), (0.05, 0.62, 0.3), PAINT)  # wing-tip plate
    for x in (side * 1.0, side * 1.75):
        z = wing_z(x) - 0.21  # the pod's axis
        hull.box((x, -0.35, z + 0.1), (0.07, 0.5, 0.2), METAL)  # pylon
        hull.cylinder((x, -0.4, z), 0.15, 0.95, METAL, axis="Y", segments=8)  # rocket pod
        hull.cylinder((x, 0.08, z), 0.15, 0.04, DARK, axis="Y", segments=8)  # the tubes' mouths
        hull.cylinder((x, -0.9, z), 0.15, 0.12, STEEL, axis="Y", segments=8, radius2=0.08)
    # Anti-tank missile on a rail under the tip, nose forward, with four small fins.
    mx, mz = side * 2.2, wing_z(2.2) - 0.2
    hull.box((mx, -0.35, mz + 0.1), (0.05, 0.4, 0.14), METAL)
    hull.cylinder((mx, -0.2, mz), 0.055, 1.1, STEEL, axis="Y", segments=6)
    hull.cylinder((mx, 0.42, mz), 0.055, 0.14, DARK, axis="Y", segments=6, radius2=0.01)
    hull.box((mx, -0.62, mz), (0.3, 0.14, 0.02), METAL)
    hull.box((mx, -0.62, mz), (0.02, 0.14, 0.3), METAL)

# Tail: the boom ends in a swept fin with a stub fin below it, a tailplane across the boom, an infra-red
# jammer on its back and an aerial.
hull.prism([(-3.5, 0.4), (-4.55, 0.42), (-4.75, 1.4), (-4.4, 1.4)], -0.05, 0.05, PAINT)
hull.prism([(-4.1, 0.4), (-4.55, 0.45), (-4.62, 0.0), (-4.35, 0.0)], -0.04, 0.04, PAINT)
hull.box((0.0, -4.1, 0.55), (1.5, 0.45, 0.05), PAINT)  # tailplane
for side in (-1, 1):
    hull.box((side * 0.78, -4.1, 0.55), (0.05, 0.5, 0.2), PAINT)  # its end plates
hull.box((0.0, -3.0, 0.72), (0.2, 0.55, 0.15), METAL)
hull.box((0.0, -2.3, 0.62), (0.03, 0.4, 0.3), METAL)

# Landing gear, tricycle: a nose leg under the cockpit and a main leg on each side of the cabin, all with
# wheels (see below).
hull.box((0.0, 1.3, -0.62), (0.08, 0.1, 0.4), METAL)
for side in (-1, 1):
    hull.box((side * 0.8, 0.45, -0.52), (0.1, 0.12, 0.42), METAL)  # leg
    hull.box((side * 0.66, 0.45, -0.4), (0.3, 0.14, 0.1), PAINT)  # the fairing it hangs from
hull_obj = hull.build()

# --------------------------------------------------------------------------------------------------
# Wheels (fixed, so part of the hull's own mesh would do, but a tyre and a rim make the gear read).
# --------------------------------------------------------------------------------------------------
gear = Part("gear")
WHEEL = 0.19
for x, y, radius, width in ((0.0, 1.3, 0.16, 0.1), (-0.8, 0.45, WHEEL, 0.14), (0.8, 0.45, WHEEL, 0.14)):
    z = GROUND + radius
    gear.cylinder((x, y, z), radius, width, DARK, axis="X", segments=10)
    gear.cylinder((x, y, z), radius * 0.55, width + 0.04, STEEL, axis="X", segments=8)
gear_obj = gear.build()
gear_obj.parent = hull_obj

# --------------------------------------------------------------------------------------------------
# Main rotor: a hub with a swashplate and five blades that droop and cone a little.
# --------------------------------------------------------------------------------------------------
rotor = Part("rotor", location=ROTOR_HEAD)
rotor.cylinder((0, 0, 0), 0.2, 0.22, METAL, segments=10)
rotor.cylinder((0, 0, -0.14), 0.3, 0.06, STEEL, segments=10)
for k in range(ROTOR_BLADES):
    a = 2 * math.pi * k / ROTOR_BLADES
    # Root sleeve, then the blade itself, raised a few degrees at its tip.
    reach = ROTOR_RADIUS
    cone = about_y(-0.05)
    turn = about_z(a)
    for start, end, width, mat in ((0.15, 0.55, 0.16, METAL), (0.5, reach, 0.22, DARK)):
        c = Vector(((start + end) / 2, 0, 0))
        position = turn @ (cone @ c)
        rotor.box(position, (end - start, width, 0.035), mat, rot=turn @ cone)
    tip = turn @ (cone @ Vector((reach - 0.2, 0, 0)))
    rotor.box(tip, (0.4, 0.22, 0.04), STEEL, rot=turn @ cone)
rotor_obj = rotor.build()

# Tail rotor on the fin's right side: three blades in the plane of the fin's side, turning about X.
tail_rotor = Part("tail_rotor", location=TAIL_ROTOR_HUB)
tail_rotor.cylinder((0, 0, 0), 0.09, 0.14, METAL, axis="X", segments=8)
for k in range(3):
    a = 2 * math.pi * k / 3
    c = Vector((0.0, 0.0, TAIL_ROTOR_RADIUS / 2 + 0.06))
    tail_rotor.box(about_x(a) @ c, (0.025, 0.12, TAIL_ROTOR_RADIUS - 0.1), DARK, rot=about_x(a))
tail_rotor_obj = tail_rotor.build()

# --------------------------------------------------------------------------------------------------
# Chin gun: a four-barrel machine gun on a ball mount under the nose. The mount (`turret`) yaws, the
# barrels (`gun`) nod and recoil along their own axis.
# --------------------------------------------------------------------------------------------------
turret = Part("turret", location=GUN_PIVOT)
turret.cylinder((0, 0, 0.07), 0.2, 0.14, METAL, segments=10)  # the ring under the belly
turret.dome((0, 0, 0), (0.17, 0.2, 0.17), PAINT, floor=-1.0, u=10, v=6)  # ball housing
turret.box((0.2, -0.05, 0.0), (0.16, 0.3, 0.18), PAINT)  # ammunition feed
turret_obj = turret.build()

gun = Part("gun", parent=turret_obj)
gun.box((0.0, 0.18, 0.0), (0.26, 0.5, 0.24), METAL)  # breech and gearbox
gun.cylinder((0.0, 0.62, 0.0), 0.11, 0.4, METAL, axis="Y", segments=8)  # barrel shroud
for dx, dz in ((-1, -1), (-1, 1), (1, -1), (1, 1)):
    gun.cylinder((dx * 0.05, (0.8 + GUN_LENGTH) / 2, dz * 0.05), 0.026, GUN_LENGTH - 0.8, DARK, axis="Y", segments=6)
gun.cylinder((0.0, GUN_LENGTH - 0.05, 0.0), 0.1, 0.06, METAL, axis="Y", segments=8)  # muzzle clamp
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
export(OUT)
everything = [hull_obj, gear_obj, rotor_obj, tail_rotor_obj, turret_obj, gun_obj]
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
    mid = Vector((0, -1.0, 0.3))
    views = {
        "side": ("ORTHO", Vector((14, -1.0, 0.3)), mid, 10.5),
        "top": ("ORTHO", Vector((0, -1.0, 14)), mid, 10.5),
        "front": ("ORTHO", Vector((0, 14, 0.3)), mid, 8.0),
        "three": ("PERSP", Vector((8.5, 8.5, 4.0)), mid, 0),
        "rear": ("PERSP", Vector((-7.5, -10.5, 3.5)), mid, 0),
        "nose": ("PERSP", Vector((3.0, 5.0, 0.4)), Vector((0, 1.8, -0.1)), 0),
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
