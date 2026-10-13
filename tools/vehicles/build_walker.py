"""
Builds the walker (an AT-ST in silhouette) and exports it to client/public/models/walker.glb.

    /Applications/Blender.app/Contents/MacOS/Blender -b -P tools/vehicles/build_walker.py -- \
        [--out client/public/models/walker.glb] [--preview DIR]

Everything is procedural and flat-shaded.

Conventions
  * Blender: Z up, the guns point along +Y, the walker's right hand side is +X. The glTF exporter turns
    that into Y up, guns along -Z, which is the game's vehicle frame (body axes +X right, +Y up, -Z forward).
  * Origin is the centre of the hull collider (a 2.0 x 2.4 x 3.6 m box, WALKER_HALF_EXTENTS).
  * Nodes the game looks up by name:
      `torso`      the upper body, origin at the hull centre; the game yaws it with the aim
      `gun`        child of `torso`, origin on WALKER_GUN_PIVOT; the game pitches it and slides it back in recoil
      `hip_L/R`    legs, origin at the hip (WALKER_FOOT_HALF sideways, the bottom of the hull); the game swings them
      `pelvis`     the fixed yoke between the hips, on the legs and not the upper body
      `knee_L/R`   child of the hip, one thigh below it
      `ankle_L/R`  child of the knee, one shin below it; the foot's sole is PAD below this origin
    Legs hang straight down in the rest pose: the game's IK sets every rotation from that.
  * Materials: `paint` is tinted per walker at runtime; `dark`, `metal`, `steel` keep their colours and
    `lamp` is unlit (viewports, muzzle glow).
"""

import math
import os
import sys

import bmesh
import bpy
from mathutils import Matrix, Vector

argv = sys.argv[sys.argv.index("--") + 1 :] if "--" in sys.argv else []


def arg(name, default=None):
    return argv[argv.index(name) + 1] if name in argv else default


HERE = os.path.dirname(os.path.abspath(__file__))
OUT = arg("--out", os.path.normpath(os.path.join(HERE, "../../client/public/models/walker.glb")))
PREVIEW = arg("--preview")

# --------------------------------------------------------------------------------------------------
# Dimensions (metres). Keep in step with WALKER_* in crates/shared/src/consts.rs and render/walker.ts.
# --------------------------------------------------------------------------------------------------
HX, HY, HZ = 1.0, 1.8, 1.2  # half extents of the hull box: sideways, along the guns, up
FOOT_HALF = 1.7
# The legs are longer than the physics' reach (WALKER_LEG_LENGTH = 4.2) so that they stand bent, knees back.
THIGH, SHIN = 2.1, 2.2
PAD = 0.3  # the foot's thickness: ankle origin to sole
GUN_PIVOT = Vector((0.0, 0.0, 0.1))  # game [0, 0.1, 0]
GUN_LENGTH = 3.7

bpy.ops.wm.read_factory_settings(use_empty=True)
scene = bpy.context.scene


def material(name, rgb):
    m = bpy.data.materials.new(name)
    m.use_nodes = True
    bsdf = m.node_tree.nodes["Principled BSDF"]
    bsdf.inputs["Base Color"].default_value = (*rgb, 1.0)
    bsdf.inputs["Roughness"].default_value = 1.0
    m.diffuse_color = (*rgb, 1.0)
    return m


MATERIALS = [
    material("paint", (0.55, 0.58, 0.58)),
    material("dark", (0.12, 0.13, 0.14)),
    material("metal", (0.26, 0.28, 0.3)),
    material("steel", (0.6, 0.62, 0.64)),
    material("lamp", (1.0, 0.5, 0.12)),
]
PAINT, DARK, METAL, STEEL, LAMP = range(5)


class Part:
    """A mesh under construction: primitives are added in the part's own frame, each with a material."""

    def __init__(self, name, location=(0.0, 0.0, 0.0), parent=None):
        self.bm = bmesh.new()
        self.name = name
        self.location = Vector(location)
        self.parent = parent

    def _tag(self, verts, mat):
        for face in {f for v in verts for f in v.link_faces}:
            face.material_index = mat

    def box(self, centre, size, mat, rx=0.0):
        m = Matrix.Translation(centre) @ Matrix.Rotation(rx, 4, "X") @ Matrix.Diagonal((*size, 1.0))
        self._tag(bmesh.ops.create_cube(self.bm, size=1.0, matrix=m)["verts"], mat)

    def cylinder(self, centre, radius, depth, mat, axis="Z", segments=10, radius2=None, squash=(1.0, 1.0)):
        """A cylinder or frustum. `squash` stretches the two cross-section axes (a slot or an ellipse)."""
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

    def dome(self, centre, scale, mat, floor, u=14, v=8):
        """The part of an ellipsoid above the horizontal plane `floor` (relative to its centre)."""
        m = Matrix.Translation(centre) @ Matrix.Diagonal((*scale, 1.0))
        verts = bmesh.ops.create_uvsphere(self.bm, u_segments=u, v_segments=v, radius=1.0, matrix=m)["verts"]
        below = [vert for vert in verts if vert.co.z < centre[2] + floor * scale[2] - 1e-4]
        bmesh.ops.delete(self.bm, geom=below, context="VERTS")
        self._tag([vert for vert in verts if vert.is_valid], mat)

    def prism(self, profile, x0, x1, mat):
        """Extrudes a closed (y, z) outline along X, from x0 to x1."""
        a = [self.bm.verts.new((x0, y, z)) for y, z in profile]
        b = [self.bm.verts.new((x1, y, z)) for y, z in profile]
        faces = [self.bm.faces.new(a), self.bm.faces.new(b[::-1])]
        for i in range(len(profile)):
            j = (i + 1) % len(profile)
            faces.append(self.bm.faces.new((a[i], a[j], b[j], b[i])))
        for face in faces:
            face.material_index = mat

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

    def ring(self, outer, inner, x0, x1, mat):
        """Extrudes the band between two (y, z) loops along X."""
        n = len(outer)
        verts = {
            (name, x, i): self.bm.verts.new((x, *loop[i]))
            for name, loop in (("o", outer), ("i", inner))
            for x in (x0, x1)
            for i in range(n)
        }
        for i in range(n):
            j = (i + 1) % n
            for x in (x0, x1):
                self.bm.faces.new((verts["o", x, i], verts["o", x, j], verts["i", x, j], verts["i", x, i])).material_index = mat
            self.bm.faces.new((verts["o", x0, i], verts["o", x0, j], verts["o", x1, j], verts["o", x1, i])).material_index = mat
            self.bm.faces.new((verts["i", x0, i], verts["i", x0, j], verts["i", x1, j], verts["i", x1, i])).material_index = mat

    def build(self):
        bmesh.ops.remove_doubles(self.bm, verts=self.bm.verts, dist=1e-4)
        bmesh.ops.recalc_face_normals(self.bm, faces=self.bm.faces)
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



# --------------------------------------------------------------------------------------------------
# Upper body: the AT-ST's head, a narrow wedge that tapers to a chin, with slit viewports under a
# brow, cheek-mounted guns, ear hatches and a rear pack
# --------------------------------------------------------------------------------------------------
torso = Part("torso")
# Lower head: side profile (y, z), widest at the back, the chin thrust forward and down.
torso.prism(
    [(-1.8, -0.95), (1.0, -1.2), (1.75, -0.75), (1.85, -0.1), (1.35, 0.35), (-1.8, 0.35)],
    -0.8,
    0.8,
    PAINT,
)
# Upper head: narrower, a raked forehead over the viewports and a long roof sloping to the back.
torso.prism(
    [(-1.7, 0.3), (1.3, 0.3), (1.55, 0.75), (0.9, 1.15), (-0.6, 1.25), (-1.7, 0.9)],
    -0.55,
    0.55,
    PAINT,
)
for side in (-1, 1):
    torso.box((side * 0.4, 1.66, 0.03), (0.4, 0.1, 0.16), LAMP)  # slit viewports
    torso.box((side * 0.4, 1.7, 0.2), (0.5, 0.16, 0.1), DARK, rx=0.4)  # their brow
    # The ear hatches on the roof's flanks, and the cheek guns: a box and a short barrel each.
    torso.cylinder((side * 0.4, -0.7, 1.27), 0.28, 0.1, METAL, segments=10)
    torso.box((side * 0.92, 0.6, -0.1), (0.3, 1.6, 0.5), METAL)
    torso.cylinder((side * 0.92, 1.7, -0.1), 0.07, 1.0, DARK, axis="Y", segments=6)
    torso.box((side * 0.6, -0.6, 0.62), (0.08, 1.8, 0.5), DARK)  # raked side armour
torso.box((0.0, 0.0, -1.2), (1.3, 3.0, 0.2), DARK)  # belly
torso.box((0.0, -2.0, 0.0), (1.0, 0.5, 1.0), DARK)  # rear pack
for side in (-1, 1):
    torso.cylinder((side * 0.35, -2.35, 0.35), 0.1, 0.5, METAL, axis="Y", segments=8)
torso.cylinder((0.0, -0.4, 1.3), 0.3, 0.1, STEEL, segments=8)  # roof hatch
torso.cylinder((0.5, -1.2, 1.75), 0.02, 1.0, METAL, segments=5)  # antenna
torso_obj = torso.build()

# The chin guns: a housing with two long barrels, which lift together about the pivot.
gun = Part("gun", location=GUN_PIVOT, parent=torso_obj)
gun.box((0.0, 1.55, -1.0), (0.9, 0.95, 0.55), DARK)  # mount under the chin
gun.box((0.0, 1.0, -0.8), (0.6, 0.6, 0.3), PAINT)
for side in (-1, 1):
    x = side * 0.3
    gun.cylinder((x, 2.8, -1.0), 0.07, GUN_LENGTH - 2.0, METAL, axis="Y", segments=8)  # barrel
    gun.cylinder((x, 2.2, -1.0), 0.12, 0.7, DARK, axis="Y", segments=8)  # shroud
    gun.cylinder((x, GUN_LENGTH - 0.07, -1.0), 0.09, 0.14, STEEL, axis="Y", segments=8)  # muzzle brake
    gun.cylinder((x, GUN_LENGTH + 0.0, -1.0), 0.05, 0.04, LAMP, axis="Y", segments=6)
gun_obj = gun.build()

# --------------------------------------------------------------------------------------------------
# Legs: long and thin like a bird's: a hip ball, thigh, a knee joint with its piston, a long shin, an
# ankle and a big three-toed foot, hanging straight down
# --------------------------------------------------------------------------------------------------
# The pelvis carries the hips and does not turn with the upper body: a yoke out to each hip.
pelvis = Part("pelvis")
pelvis.box((0.0, 0.0, -HZ - 0.12), (2 * FOOT_HALF - 0.4, 1.3, 0.5), PAINT)
pelvis.box((0.0, 0.0, -HZ + 0.1), (1.4, 1.6, 0.2), DARK)
pelvis.box((0.0, 0.55, -HZ - 0.12), (1.2, 0.2, 0.4), METAL)
pelvis_obj = pelvis.build()

leg_objects = [pelvis_obj]
for side, label in ((-1, "L"), (1, "R")):
    hip = Part(f"hip_{label}", location=(side * FOOT_HALF, 0.0, -HZ))
    hip.cylinder((0, 0, 0), 0.42, 0.7, DARK, axis="X", segments=10)
    hip.cylinder((side * 0.36, 0, 0), 0.3, 0.14, METAL, axis="X", segments=8)
    hip.taper((0.0, 0.0, -THIGH / 2 - 0.05), (0.5, 0.6), (0.4, 0.5), THIGH - 0.5, PAINT)  # thigh
    hip.cylinder((0.0, 0.34, -THIGH / 2), 0.06, THIGH - 0.6, STEEL, segments=6)  # front piston
    hip.box((0.0, -0.3, -0.7), (0.3, 0.12, 0.9), DARK)
    hip_obj = hip.build()

    knee = Part(f"knee_{label}", location=(0.0, 0.0, -THIGH), parent=hip_obj)
    knee.cylinder((0, 0, 0), 0.32, 0.6, METAL, axis="X", segments=10)
    knee.box((0.0, -0.32, 0.0), (0.45, 0.28, 0.6), PAINT, rx=0.15)  # kneecap: the knee points back
    knee.taper((0.0, 0.0, -SHIN / 2 - 0.05), (0.38, 0.48), (0.28, 0.36), SHIN - 0.5, DARK)  # shin
    knee.cylinder((0.0, 0.3, -SHIN / 2), 0.05, SHIN - 0.6, STEEL, segments=6)  # front piston
    knee_obj = knee.build()

    ankle = Part(f"ankle_{label}", location=(0.0, 0.0, -SHIN), parent=knee_obj)
    ankle.cylinder((0, 0, 0), 0.26, 0.55, METAL, axis="X", segments=8)
    ankle.box((0.0, 0.0, -PAD / 2), (0.55, 0.7, PAD), DARK)  # the pad's heel block
    for angle in (-0.55, 0.0, 0.55):
        ankle.box((math.sin(angle) * 0.5, 0.3 + math.cos(angle) * 0.5, -PAD * 0.4), (0.26, 1.1, PAD * 0.8), PAINT if angle == 0.0 else DARK)
    ankle.box((0.0, -0.5, -PAD * 0.4), (0.3, 0.5, PAD * 0.8), DARK)  # heel spur
    ankle_obj = ankle.build()
    leg_objects += [hip_obj, knee_obj, ankle_obj]

# --------------------------------------------------------------------------------------------------
# Export
# --------------------------------------------------------------------------------------------------
def export():
    os.makedirs(os.path.dirname(OUT), exist_ok=True)
    bpy.ops.object.select_all(action="SELECT")
    kwargs = dict(
        filepath=OUT,
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


export()
everything = [torso_obj, gun_obj, *leg_objects]
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
    scene.render.resolution_x, scene.render.resolution_y = 900, 700
    cam_data = bpy.data.cameras.new("cam")
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    views = {
        "side": ("ORTHO", Vector((14, 0, -2.5)), Vector((0, 0, -2.5)), 11.0),
        "front": ("ORTHO", Vector((0, -14, -2.5)), Vector((0, 0, -2.5)), 11.0),
        "three": ("PERSP", Vector((11, -11, 2.0)), Vector((0, 0, -2.5)), 0),
        "rear": ("PERSP", Vector((-9, 11, 2.5)), Vector((0, 0, -2.5)), 0),
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
