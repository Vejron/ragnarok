"""
Surveys downloaded character models for the bake-off (docs/CHARACTER-LOOKS.md): for every .glb under a
directory it prints the triangle count, the skeleton's bones and rest pose, the materials and the
animation clips, and renders the model standing and part way through a run.

    Blender -b -P tools/character/survey.py -- DIR OUT [--report OUT/report.json]

It only reads the models. Each model is scaled to 1.72 m for the renders, so they compare side by
side. The renders are `OUT/<pack>__<name>_<view>.png`.
"""

import json
import math
import os
import sys

import bpy
from mathutils import Vector

argv = sys.argv[sys.argv.index("--") + 1 :]
SRC, OUT = argv[0], argv[1]
os.makedirs(OUT, exist_ok=True)
HEIGHT = 1.72


def reset():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    scene = bpy.context.scene
    scene.render.engine = "BLENDER_WORKBENCH"
    scene.display.shading.light = "STUDIO"
    scene.display.shading.color_type = "TEXTURE"
    scene.render.resolution_x, scene.render.resolution_y = 300, 420
    scene.render.film_transparent = False
    world = bpy.data.worlds.new("w")
    world.color = (0.75, 0.78, 0.82)
    scene.world = world
    return scene


def bounds(objects):
    lo, hi = Vector((1e9, 1e9, 1e9)), Vector((-1e9, -1e9, -1e9))
    deps = bpy.context.evaluated_depsgraph_get()
    for o in objects:
        if o.type != "MESH":
            continue
        ev = o.evaluated_get(deps)
        me = ev.to_mesh()
        for v in me.vertices:
            p = ev.matrix_world @ v.co
            lo = Vector(map(min, lo, p))
            hi = Vector(map(max, hi, p))
        ev.to_mesh_clear()
    return lo, hi


def rest_pose(arm):
    """How the arms lie in the rest pose: 'T' (out level), 'A' (out and down) or 'down'."""
    for b in arm.data.bones:
        n = b.name.lower()
        if ("upperarm" in n or "upper_arm" in n or "arm.l" in n or n.endswith("arml") or "leftarm" in n or "shoulder.l" in n) and "fore" not in n and "twist" not in n:
            d = (b.tail_local - b.head_local).normalized()
            drop = math.degrees(math.asin(max(-1, min(1, -d.z))))
            return "T" if drop < 15 else "A" if drop < 60 else "down", round(drop)
    return "?", None


def survey(path):
    scene = reset()
    bpy.ops.import_scene.gltf(filepath=path)
    objects = list(scene.objects)
    meshes = [o for o in objects if o.type == "MESH"]
    arms = [o for o in objects if o.type == "ARMATURE"]
    tris = sum(sum(len(p.vertices) - 2 for p in o.data.polygons) for o in meshes)
    textures = sorted({n.image.name for m in bpy.data.materials if m.use_nodes for n in m.node_tree.nodes if n.type == "TEX_IMAGE" and n.image})
    info = {
        "file": os.path.relpath(path, SRC),
        "tris": tris,
        "meshes": len(meshes),
        "materials": sorted(m.name for m in bpy.data.materials),
        "textures": textures,
        "bones": [b.name for b in arms[0].data.bones] if arms else [],
        "rest": rest_pose(arms[0]) if arms else None,
        "clips": sorted(a.name for a in bpy.data.actions),
    }
    # Scale to a common height, feet on the ground.
    lo, hi = bounds(meshes)
    k = HEIGHT / max(hi.z - lo.z, 1e-3)
    root = [o for o in objects if o.parent is None]
    for o in root:
        o.scale = o.scale * k
        o.location = (o.location - Vector((0, 0, lo.z))) * k
    bpy.context.view_layer.update()
    info["height"] = round(hi.z - lo.z, 3)

    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = 2.2
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    # These packs' characters face -Y in Blender after import: look at them from there.
    views = {"front": Vector((0, -6, 0.9)), "three": Vector((-4, -5, 2.4)), "side": Vector((6, 0, 0.9))}
    stem = info["file"].replace(os.sep, "__").rsplit(".", 1)[0]

    def shoot(tag):
        for name, pos in views.items():
            cam.location = pos
            cam.rotation_euler = (Vector((0, 0, 0.88)) - pos).to_track_quat("-Z", "Y").to_euler()
            scene.render.filepath = os.path.join(OUT, f"{stem}_{tag}_{name}.png")
            bpy.ops.render.render(write_still=True)

    shoot("rest")
    run = next((a for a in bpy.data.actions if a.name.lower().split("|")[-1] in ("run", "running", "run_forward", "jog_fwd", "jog_fwd_loop")), None)
    run = run or next((a for a in bpy.data.actions if "run" in a.name.lower() and "gun" not in a.name.lower()), None)
    if run and arms:
        arms[0].animation_data_create()
        arms[0].animation_data.action = run
        start, end = run.frame_range
        scene.frame_set(int(start + (end - start) * 0.3))
        info["run_clip"] = run.name
        shoot("run")
    return info


report = []
for dirpath, _, files in os.walk(SRC):
    for f in sorted(files):
        if f.lower().endswith(".glb"):
            try:
                report.append(survey(os.path.join(dirpath, f)))
                r = report[-1]
                print(f"SURVEY {r['file']}: {r['tris']} tris, {len(r['bones'])} bones, rest {r['rest']}, {len(r['clips'])} clips, textures {r['textures']}")
            except Exception as e:  # a model that will not import is reported, not fatal
                print(f"SURVEY {f}: failed: {e}")
json.dump(report, open(os.path.join(OUT, "report.json"), "w"), indent=1)
