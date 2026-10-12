"""
Builds the game's sound effects (client/public/sfx/*.m4a) from CC0 recordings.

    python3 tools/sfx/build_sfx.py --firearms DIR --impacts DIR --explosions DIR [--out client/public/sfx]

Needs numpy and soundfile (pip), and macOS `afconvert` for the AAC encode.

Sources (all CC0, see client/public/sfx/CREDITS.md)
  --firearms    "The Free Firearm Sound Library", the "Prepared SFX Library" folder (96 kHz stereo, a few
                shots per file, ~4 s apart)
  --impacts     "Impacts/gunshots" by Eldritch Grim: the Impacts folder
  --explosions  a folder holding synthetic_explosion_1.flac (qubodup) and explosion01..04.wav (soundpack 03)

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


def encode(x, out, kbps=96):
    with tempfile.TemporaryDirectory() as tmp:
        wav = os.path.join(tmp, "x.wav")
        sf.write(wav, normalise(x), RATE, subtype="PCM_16")
        subprocess.run(["afconvert", wav, out, "-f", "m4af", "-d", "aac", "-b", str(kbps * 1000), "-q", "127"], check=True)
    print(f"{os.path.basename(out):16s} {len(x) / RATE:4.2f}s {os.path.getsize(out) // 1024:4d}K")


def main():
    p = argparse.ArgumentParser()
    p.add_argument("--firearms", required=True)
    p.add_argument("--impacts", required=True)
    p.add_argument("--explosions", required=True)
    p.add_argument("--out", default="client/public/sfx")
    a = p.parse_args()
    os.makedirs(a.out, exist_ok=True)
    f = lambda name: glob.glob(os.path.join(a.firearms, name))[0]
    out = lambda name: os.path.join(a.out, name + ".m4a")

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
