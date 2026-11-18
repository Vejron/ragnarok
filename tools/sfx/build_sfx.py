"""
Builds the game's sound effects (client/public/sfx/*.m4a) from CC0 recordings.

    python3 tools/sfx/build_sfx.py [--firearms DIR --explosions DIR] [--impacts DIR] [--water DIR] [--breaking DIR] [--out client/public/sfx]

Each source is optional: only the sounds made from the ones given are written, so one set can be remade
without the others' sources at hand. Needs numpy and soundfile (pip), and macOS `afconvert` for the AAC
encode.

Sources (all CC0, see client/public/sfx/CREDITS.md)
  --firearms    "The Free Firearm Sound Library", the "Prepared SFX Library" folder (96 kHz stereo, a few
                shots per file, ~4 s apart)
  --explosions  a folder holding synthetic_explosion_1.flac (qubodup) and explosion01..04.wav (soundpack 03)
                (with --firearms, whose shotgun blasts deepen the booms)
  --impacts     "Impacts/gunshots" by Eldritch Grim: the Impacts folder (rounds striking earth and stone)
  --water       "40 CC0 water / splash / slime SFX" by rubberduck: the unpacked folder (splashes)
  --breaking    "75 CC0 breaking / falling / hit sfx" by rubberduck: the unpacked folder (glass, debris,
                a building coming down)

Every output is mono, 48 kHz, peak-normalised, with its first shot cut out at the transient (a few ms of
lead-in) and a short fade at the tail. Big booms are a shotgun / rifle blast slowed to a fraction of its
speed, which drops it by octaves and stretches the tail, layered under the synthetic explosion.
"""
import argparse
import glob
import os
import subprocess
import tempfile

import numpy as np
import soundfile as sf

RATE = 48000


def load(path):
    x, sr = sf.read(path, dtype="float32")
    if x.ndim > 1:
        x = x.mean(axis=1)
    if sr != RATE:
        x = resample(x, sr / RATE)
    return x


def resample(x, step):
    """Reads `x` at `step` input samples per output sample (>1 up, <1 down in pitch)."""
    if step > 1:  # a crude box lowpass first, so nothing folds back
        k = int(round(step))
        n = len(x) // k * k
        x = x[:n].reshape(-1, k).mean(axis=1)
        step /= k
    pos = np.arange(0, len(x) - 1, step)
    return np.interp(pos, np.arange(len(x)), x).astype("float32")


def first_onset(x, threshold=0.25):
    w = int(RATE * 0.004)
    env = np.convolve(np.abs(x), np.ones(w) / w, "same")
    return int(np.argmax(env > env.max() * threshold))


def next_onset(x, start, gap=0.3, threshold=0.25):
    w = int(RATE * 0.004)
    env = np.convolve(np.abs(x), np.ones(w) / w, "same")
    thr = env.max() * threshold
    idx = np.flatnonzero(env[start + int(RATE * gap):] > thr)
    return start + int(RATE * gap) + int(idx[0]) if len(idx) else len(x)


def shot(path, length, lead=0.004, fade=0.25):
    """The first shot of a recording: from just before its transient, `length` s at most."""
    x = load(path)
    a = max(0, first_onset(x) - int(RATE * lead))
    b = min(next_onset(x, a) - int(RATE * 0.05), a + int(RATE * length), len(x))
    return tail(x[a:b], fade)


def whole(path, length, lead=0.004, fade=0.25):
    """A recording of one event (a splash, a shatter: many transients in a row), from just before it
    starts to its end, `length` s at most."""
    x = load(path)
    a = max(0, first_onset(x) - int(RATE * lead))
    return tail(x[a:min(len(x), a + int(RATE * length))], fade)


def tail(x, fade):
    n = min(len(x), int(RATE * fade))
    x = x.copy()
    x[len(x) - n:] *= np.linspace(1, 0, n) ** 2
    return x


def lowpass(x, cutoff):
    """One-pole lowpass, run twice (12 dB/oct)."""
    a = 1 - np.exp(-2 * np.pi * cutoff / RATE)
    y = np.empty_like(x)
    for _ in range(2):
        s = 0.0
        for i, v in enumerate(x):
            s += a * (v - s)
            y[i] = s
        x = y.copy()
    return y


def mix(*layers):
    n = max(len(l) + int(RATE * d) for l, d, _ in layers)
    out = np.zeros(n, dtype="float32")
    for l, d, g in layers:
        i = int(RATE * d)
        out[i:i + len(l)] += l * g
    return out


def normalise(x, peak=0.9):
    return x * (peak / max(1e-6, float(np.abs(x).max())))


def loud(x, level):
    """`x` made as loud as `level` over its loudest 50 ms (RMS), its peaks softly limited: a splash or a
    shatter is one sharp click over a quieter body, and normalised by its peak the body is barely heard."""
    w = int(RATE * 0.05)
    env = np.sqrt(np.convolve(x ** 2, np.ones(w) / w, "same"))
    y = x * (level / max(1e-6, float(env.max())))
    return (0.88 * np.tanh(y / 0.88)).astype("float32")


def encode(x, out, kbps=96, level=None):
    """Writes `x` as AAC: normalised to its peak, or to `level` of loudness (`loud`)."""
    with tempfile.TemporaryDirectory() as tmp:
        wav = os.path.join(tmp, "x.wav")
        sf.write(wav, normalise(x) if level is None else loud(x, level), RATE, subtype="PCM_16")
        subprocess.run(["afconvert", wav, out, "-f", "m4af", "-d", "aac", "-b", str(kbps * 1000), "-q", "127"], check=True)
    print(f"{os.path.basename(out):16s} {len(x) / RATE:4.2f}s {os.path.getsize(out) // 1024:4d}K")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--firearms")
    p.add_argument("--impacts")
    p.add_argument("--explosions")
    p.add_argument("--water")
    p.add_argument("--breaking")
    p.add_argument("--out", default="client/public/sfx")
    a = p.parse_args()
    os.makedirs(a.out, exist_ok=True)
    out = lambda name: os.path.join(a.out, name + ".m4a")
    if a.firearms and a.explosions:
        guns(a, out)
    if a.impacts:
        impacts(a.impacts, out)
    if a.water:
        water(a.water, out)
    if a.breaking:
        breaking(a.breaking, out)


def deepen(x, step, cutoff):
    """`x` slowed to `step` of its speed (lower and longer), with its low end layered under it for weight."""
    slow = resample(x, step)
    return mix((slow, 0, 0.7), (lowpass(slow, cutoff), 0, 1.0))


def impacts(folder, out):
    """Rounds striking something: dull, low thuds for earth, short bright cracks for stone and brick."""
    path = lambda name: os.path.join(folder, name)
    for i, name in enumerate(["Impact 8.wav", "Impact 6.wav", "Impact 9.wav"], 1):
        encode(shot(path(name), 0.5, fade=0.2), out(f"hit_dirt_{i}"))
    for i, name in enumerate(["Impact 7.wav", "Impact 4.wav", "Impact 2.wav"], 1):
        encode(shot(path(name), 0.45, fade=0.2), out(f"hit_stone_{i}"))


def water(folder, out):
    """Splashes: short ones for a round or a body, the longest and darkest slowed and weighted for a vehicle or a fallen span."""
    path = lambda name: os.path.join(folder, name)
    for i, name in enumerate(["splash_06.ogg", "splash_10.ogg", "splash_14.ogg", "splash_15.ogg"], 1):
        encode(whole(path(name), 0.8, fade=0.25), out(f"splash_small_{i}"), level=0.4)
    for i, (name, step) in enumerate([("splash_04.ogg", 0.62), ("splash_11.ogg", 0.7), ("splash_02.ogg", 0.55)], 1):
        encode(tail(deepen(whole(path(name), 1.4, fade=0.4), step, 500), 0.6), out(f"splash_big_{i}"), level=0.5)


def breaking(folder, out):
    """Window glass breaking, stone and brick landing, and the crash of a section of wall coming down."""
    path = lambda name: os.path.join(folder, name)
    for i, n in enumerate([2, 4, 5, 6], 1):
        encode(whole(path(f"bfh1_glass_breaking_0{n}.ogg"), 1.0, fade=0.3), out(f"glass_{i}"), level=0.35)
    for i, n in enumerate([1, 3, 7, 8], 1):
        encode(whole(path(f"bfh1_rock_falling_0{n}.ogg"), 0.9, fade=0.25), out(f"debris_{i}"), level=0.3)
    for i, (fall, rock) in enumerate([("bfh1_falling_04.ogg", "bfh1_rock_falling_01.ogg"), ("bfh1_falling_07.ogg", "bfh1_breaking_01.ogg")], 1):
        body = deepen(load(path(fall)), 0.6, 400)
        rubble = deepen(load(path(rock)), 0.75, 900)
        encode(tail(mix((body, 0, 1.0), (rubble, 0.08, 0.7)), 0.7), out(f"collapse_{i}"), level=0.45)


def guns(a, out):
    f = lambda name: glob.glob(os.path.join(a.firearms, name))[0]

    # Hand guns and rifles: close recordings with the room tail kept (it is what makes them sound outdoors).
    for i, name in enumerate(["AK-47/C_28P.wav", "AK-47/C_29P.wav", "AK-47/C_31P.wav", "AR-15/D_24P.wav"], 1):
        encode(shot(f(name), 2.2), out(f"rifle_{i}"))
    for i, name in enumerate(["Ruger Single Six/S_11P.wav", "Ruger Single Six/S_14P.wav"], 1):
        encode(shot(f(name), 1.8), out(f"revolver_{i}"))
    for i, name in enumerate(["Mosin Nagant/M_21P.wav", "Mosin Nagant/M_26P.wav"], 1):
        encode(shot(f(name), 3.0, fade=0.6), out(f"sniper_{i}"))
    # Machine guns fire a round every twelfth of a second: a short crack with no tail to pile up.
    for i, name in enumerate(["PPSh/P_22P.wav", "PPSh/P_34P.wav"], 1):
        encode(shot(f(name), 0.35, fade=0.12), out(f"mg_{i}"))

    # Explosions.
    expl = [load(os.path.join(a.explosions, "explosion0%d.wav" % k)) for k in (1, 2, 3, 4)]
    synth = load(os.path.join(a.explosions, "synthetic_explosion_1.flac"))
    for i, x in enumerate(expl, 1):
        encode(tail(x, 0.15), out(f"blast_{i}"))
    big = shot(f("Mossberg/N_26P.wav"), 3.0, fade=0.5)
    huge = shot(f("Model 12/K_17P.wav"), 3.0, fade=0.5)
    for i, (src, step) in enumerate([(big, 0.45), (huge, 0.35)], 1):
        slow = resample(src, step)[: int(RATE * 3.5)]
        body = lowpass(slow, 600)
        encode(tail(mix((slow, 0, 0.5), (body, 0, 1.0), (synth, 0, 0.8), (expl[0], 0.02, 0.5)), 0.8), out(f"boom_{i}"))
    # A tank's or helicopter's gun: a rifle round slowed a little, thumped up with the low end.
    for i, name in enumerate(["Model 1894/L_17P.wav", "Marlin 336/I_17P.wav"], 1):
        src = shot(f(name), 2.0, fade=0.4)
        slow = resample(src, 0.6)
        encode(tail(mix((slow, 0, 0.7), (lowpass(slow, 500), 0, 1.0)), 0.5), out(f"cannon_{i}"))


if __name__ == "__main__":
    main()
