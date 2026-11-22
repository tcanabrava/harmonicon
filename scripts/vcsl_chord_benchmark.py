# SPDX-License-Identifier: MIT
# /// script
# dependencies = ["numpy>=2"]
# ///
"""Build a C harmonica chord fixture from VCSL's recorded soft blow sustains."""

import argparse
import json
import subprocess
import wave
from pathlib import Path
from urllib.parse import quote
from urllib.request import urlopen

import numpy as np

SOURCE = ("https://raw.githubusercontent.com/sgossner/VCSL/c1ea7bcc3c7309650ab0da9d15c9cd1fbc4a4c7e/"
          "Aerophones/Free Aerophones/Harmonica-Hohner-Special20-C/Sustains/Soft/")
RATE = 44_100


def read_reed(label):
    url = quote(SOURCE + f"Hohner-Special20_Soft_{label}.wav", safe=":/")
    with urlopen(url, timeout=60) as response:
        pcm = subprocess.check_output(["ffmpeg", "-v", "error", "-i", "pipe:0", "-ar", str(RATE),
                                       "-ac", "1", "-f", "f32le", "pipe:1"], input=response.read())
    # The octave numbers on these VCSL files are one below scientific pitch.
    excerpt = np.frombuffer(pcm, dtype="<f4")[RATE // 2:RATE + RATE // 2]
    return excerpt / np.max(np.abs(excerpt))


def prepare(directory):
    notes = "C3 E3 G3 C4 E4 G4 C5 E5 G5 C6".split()
    samples = [read_reed(note) for note in notes]
    root = Path(__file__).resolve().parents[1]
    chart = json.loads((root / "assets/debug_songs/synthetic_c_chords/expected.harpchart").read_text())
    chart["metadata"].update(author="Versilian Community Sample Library", source=quote(SOURCE, safe=":/"),
                             license="CC0-1.0", description="Mixtures of recorded Special 20 soft sustains")
    chart["song"].update(title="Special 20 soft blow chords", artist="VCSL", genre="Practice")
    chart["track"] = []
    blow = chart["harmonica"]["layout"]["blow"]
    groups = ([range(i, i + 3) for i in range(8)] + [(i, i + 3) for i in range(7)]
              + [range(i, i + 4) for i in (0, 3, 6)])
    parts = []
    for holes in groups:
        for bass_level in (1, 0.4):
            mixture = np.zeros(RATE)
            for hole in holes:
                reed = samples[hole]
                mixture[:len(reed)] += reed * (bass_level if hole == holes[0] else 1)
            mixture *= 0.75 / np.max(np.abs(mixture))
            mixture[:441] *= np.linspace(0, 1, 441)
            mixture[-441:] *= np.linspace(1, 0, 441)
            chart["track"].append(dict(time=len(parts) * 1.5, duration=1, play_mode="chord",
                                       events=[dict(hole=h + 1, action="blow", note=blow[h]) for h in holes]))
            parts.append(np.concatenate([mixture, np.zeros(RATE // 2)]))
    directory.mkdir(parents=True, exist_ok=True)
    with wave.open(str(directory / "recording.wav"), "wb") as wav:
        wav.setparams((1, 2, RATE, 0, "NONE", ""))
        wav.writeframes((np.concatenate(parts) * 32767).astype("<i2").tobytes())
    (directory / "expected.harpchart").write_text(json.dumps(chart, indent=2) + "\n")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    prepare(parser.parse_args().directory)
