"""
Takes out of a `.glb` the animation tracks that say nothing, after Blender has exported it.

Blender writes a translation, a rotation and a scale track for every bone in every clip, though most
bones never move from where they rest or change size: the soldier's 177 clips came to 19,000 tracks,
and the JSON that lists them to half of a 5.8 MB file, parsed on the page's main thread before the
game's first frame (docs/LOAD-TIME.md).

A track is dropped when it is one of a set that changes nothing: the tracks of one property of one
bone (its translation, say), where in every clip that has one the track holds a single value and that
value is the bone's rest pose. The first clip's track of the set is kept. The client's `Animator`
(engine/src/animation/animator.ts) tops a property up with its rest value wherever the playing clips
do not drive it, so with one track left it still writes the property every frame, and writes the same
value: nothing that is drawn changes, whatever the blend.

What the dropped tracks pointed at (samplers, accessors, buffer views, bytes) goes with them.

    python3 tools/gltf_prune.py client/public/models/player.glb [more.glb ...]

The model build scripts call `prune` on what they export.
"""

import json
import struct
import sys

# How far a track's keys may be from each other, and from the rest pose, to count as that pose.
EPSILON = 1e-5

REST = {"translation": [0.0, 0.0, 0.0], "rotation": [0.0, 0.0, 0.0, 1.0], "scale": [1.0, 1.0, 1.0]}
SIZE = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}
FLOAT = 5126


def read_glb(path):
    with open(path, "rb") as f:
        data = f.read()
    magic, version, _ = struct.unpack_from("<4sII", data, 0)
    if magic != b"glTF" or version != 2:
        raise ValueError(f"{path} is not a glTF 2 binary")
    at = 12
    doc, binary = None, b""
    while at < len(data):
        length, kind = struct.unpack_from("<I4s", data, at)
        chunk = data[at + 8 : at + 8 + length]
        if kind == b"JSON":
            doc = json.loads(chunk)
        elif kind == b"BIN\x00":
            binary = chunk
        at += 8 + length
    return doc, binary


def write_glb(path, doc, binary):
    text = json.dumps(doc, separators=(",", ":")).encode()
    text += b" " * (-len(text) % 4)
    binary += b"\x00" * (-len(binary) % 4)
    with open(path, "wb") as f:
        f.write(struct.pack("<4sII", b"glTF", 2, 12 + 8 + len(text) + 8 + len(binary)))
        f.write(struct.pack("<I4s", len(text), b"JSON"))
        f.write(text)
        f.write(struct.pack("<I4s", len(binary), b"BIN\x00"))
        f.write(binary)


def floats(doc, binary, index):
    """An accessor's values as a flat list, or None if it is not plain floats."""
    accessor = doc["accessors"][index]
    if accessor["componentType"] != FLOAT or "sparse" in accessor or "bufferView" not in accessor:
        return None
    view = doc["bufferViews"][accessor["bufferView"]]
    if view.get("byteStride") not in (None, 4 * SIZE[accessor["type"]]):
        return None
    count = accessor["count"] * SIZE[accessor["type"]]
    return struct.unpack_from(f"<{count}f", binary, view.get("byteOffset", 0) + accessor.get("byteOffset", 0))


def at_rest(values, rest, path):
    """Whether every key of a track is the rest pose. A rotation and its negative are the same turn."""
    size = len(rest)
    if len(values) % size:
        return False
    for start in range(0, len(values), size):
        key = values[start : start + size]
        same = all(abs(a - b) <= EPSILON for a, b in zip(key, rest))
        flipped = path == "rotation" and all(abs(a + b) <= EPSILON for a, b in zip(key, rest))
        if not (same or flipped):
            return False
    return True


def reindex(doc, name, used, refer):
    """Drops the entries of `doc[name]` not in `used`, and renumbers what refers to them through `refer`."""
    kept = sorted(used)
    place = {old: new for new, old in enumerate(kept)}
    doc[name] = [doc[name][old] for old in kept]
    refer(place)


def prune(path, out=None):
    """Prunes `path` in place (or into `out`). Returns (tracks before, tracks after, bytes before, bytes after)."""
    doc, binary = read_glb(path)
    if len(doc.get("buffers", [])) != 1 or "uri" in doc["buffers"][0]:
        raise ValueError(f"{path}: expected one buffer, in the file")
    before = sum(len(a["channels"]) for a in doc.get("animations", []))
    size_before = 12 + 8 + len(json.dumps(doc, separators=(",", ":"))) + 8 + len(binary)

    # Every track of each (bone, property), in the clips' order, and whether all of them rest.
    sets = {}
    for animation in doc.get("animations", []):
        for channel in animation["channels"]:
            node, path_ = channel["target"].get("node"), channel["target"]["path"]
            if node is None or path_ not in REST:
                continue
            sampler = animation["samplers"][channel["sampler"]]
            values = None if sampler.get("interpolation") == "CUBICSPLINE" else floats(doc, binary, sampler["output"])
            rest = doc["nodes"][node].get(path_, REST[path_])
            resting = values is not None and at_rest(values, rest, path_)
            entry = sets.setdefault((node, path_), {"resting": True, "first": channel})
            entry["resting"] = entry["resting"] and resting
    drop = set()
    for animation in doc.get("animations", []):
        for channel in animation["channels"]:
            entry = sets.get((channel["target"].get("node"), channel["target"]["path"]))
            if entry and entry["resting"] and entry["first"] is not channel:
                drop.add(id(channel))
    for animation in doc.get("animations", []):
        animation["channels"] = [c for c in animation["channels"] if id(c) not in drop]
        used = {c["sampler"] for c in animation["channels"]}
        kept = sorted(used)
        place = {old: new for new, old in enumerate(kept)}
        animation["samplers"] = [animation["samplers"][old] for old in kept]
        for channel in animation["channels"]:
            channel["sampler"] = place[channel["sampler"]]
    # A clip is never left with nothing: the client finds its length in its tracks.
    if any(not a["channels"] for a in doc.get("animations", [])):
        raise ValueError(f"{path}: a clip would be left with no track")

    # The accessors still referred to, then the buffer views, then the bytes.
    def accessor_refs():
        for mesh in doc.get("meshes", []):
            for primitive in mesh["primitives"]:
                yield primitive["attributes"]
                if "indices" in primitive:
                    yield primitive, "indices"
                for target in primitive.get("targets", []):
                    yield target
        for skin in doc.get("skins", []):
            if "inverseBindMatrices" in skin:
                yield skin, "inverseBindMatrices"
        for animation in doc.get("animations", []):
            for sampler in animation["samplers"]:
                yield sampler, "input"
                yield sampler, "output"

    def each(refs):
        for ref in refs:
            if isinstance(ref, tuple):
                yield ref
            else:
                for key in ref:
                    yield ref, key

    used = {holder[key] for holder, key in each(accessor_refs())}

    def renumber_accessors(place):
        for holder, key in each(accessor_refs()):
            holder[key] = place[holder[key]]

    reindex(doc, "accessors", used, renumber_accessors)

    def view_refs():
        for accessor in doc["accessors"]:
            if "sparse" in accessor:
                raise ValueError(f"{path}: sparse accessors are not handled")
            if "bufferView" in accessor:
                yield accessor, "bufferView"
        for image in doc.get("images", []):
            if "bufferView" in image:
                yield image, "bufferView"

    used = {holder[key] for holder, key in view_refs()}

    def renumber_views(place):
        for holder, key in view_refs():
            holder[key] = place[holder[key]]

    reindex(doc, "bufferViews", used, renumber_views)

    packed = bytearray()
    for view in doc["bufferViews"]:
        start = view.get("byteOffset", 0)
        packed += b"\x00" * (-len(packed) % 4)
        view["byteOffset"] = len(packed)
        packed += binary[start : start + view["byteLength"]]
    doc["buffers"][0]["byteLength"] = len(packed)

    write_glb(out or path, doc, bytes(packed))
    after = sum(len(a["channels"]) for a in doc.get("animations", []))
    size_after = 12 + 8 + len(json.dumps(doc, separators=(",", ":"))) + 8 + len(packed)
    return before, after, size_before, size_after


if __name__ == "__main__":
    for name in sys.argv[1:]:
        tracks, left, size, smaller = prune(name)
        print(f"{name}: {tracks} tracks -> {left}, {size / 1e6:.2f} MB -> {smaller / 1e6:.2f} MB")
