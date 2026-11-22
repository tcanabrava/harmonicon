# SPDX-License-Identifier: MIT
"""Replay recorded chord exercises. Requires ffmpeg and cargo.

Usage: python scripts/guitar_chord_trainer_benchmark.py OUTPUT_DIRECTORY

Isolated Guitar Chords by rodriler (CC BY 4.0)
https://huggingface.co/datasets/rodriler/isolated-guitar-chords
Nylon Guitar Open Chords by SpeedY (CC0), from the FSD50K dev split
https://freesound.org/people/SpeedY/packs/472/
"""

import argparse
import json
import os
import subprocess
from pathlib import Path
from tempfile import TemporaryDirectory
from urllib.parse import quote
from urllib.request import urlopen

DATASET = "rodriler/isolated-guitar-chords"
REVISION = "07371b25b9cdd830638c4ffd5ed8b8a150d7f696"
FSD50K = ("https://huggingface.co/datasets/Fhrozen/FSD50k/resolve/"
          "ccf1acaff12f3f4a4c10052dddafbb6d22152c9b/clips/dev/")
TARGETS = {"A", "Am", "C", "D", "Dm", "E", "Em", "G"}


def recordings():
    # Test takes 1 and 4 are single downstrokes with different fingerings.
    # Use the published paths because some file names contain spaces.
    tree = f"https://huggingface.co/api/datasets/{DATASET}/tree/{REVISION}/data/Test"
    with urlopen(tree + "?recursive=true&limit=1000", timeout=60) as response:
        files = json.load(response)
    for file in files:
        path = Path(file["path"])
        if path.name.endswith(("_1.wav", "_4.wav", "noise_val.wav")):
            label = path.parent.name
            url = f"https://huggingface.co/datasets/{DATASET}/resolve/{REVISION}/{quote(path.as_posix())}"
            yield path.stem, label if label in TARGETS else None, url
    # Full Em, partial A/C shapes, suspended or added-tone chords.
    for sound in (8493, 8451, 8460, 8470, 8481, 8485, 8487):
        yield f"SpeedY-{sound}", "Em" if sound == 8493 else None, f"{FSD50K}{sound}.wav"


def prepare(directory):
    directory.mkdir(parents=True, exist_ok=True)
    cases = {}
    for name, label, url in recordings():
        cases[name] = label
        path = directory / f"{name}.wav"
        if path.exists():
            continue
        with urlopen(url, timeout=60) as response, TemporaryDirectory(dir=directory) as tmp:
            wav = Path(tmp) / "mono.wav"
            subprocess.run(["ffmpeg", "-v", "error", "-i", "pipe:0", "-ac", "1",
                            "-ar", "44100", "-c:a", "pcm_s16le", str(wav)],
                           input=response.read(), check=True)
            wav.replace(path)
    (directory / "chords.json").write_text(json.dumps(cases))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    prepare(args.directory)
    subprocess.run(["cargo", "test", "-p", "harmonicon-gameplay", "--lib", "recorded_open_chords",
                    "--", "--ignored", "--nocapture"], cwd=Path(__file__).resolve().parents[1],
                   env=dict(os.environ, HARMONICON_CHORD_RECORDINGS=str(args.directory.resolve())),
                   check=True)
