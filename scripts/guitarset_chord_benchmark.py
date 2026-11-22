# SPDX-License-Identifier: MIT
"""Prepare GuitarSet v1.1 microphone performances for note_bench.

Usage: python scripts/guitarset_chord_benchmark.py OUTPUT_DIRECTORY
Requires ffmpeg on PATH. Audio and annotations are CC BY 4.0, Xi et al.,
https://zenodo.org/records/3371780. The Hugging Face mirror exposes individual
recordings, so this benchmark doesn't need the complete audio archives.
"""

import argparse
import json
import subprocess
from pathlib import Path
from urllib.parse import urlencode
from urllib.request import urlopen

# All comp takes in the upstream JIT-MPE test split: player 05 and the last
# five progressions in lexicographic order. Verify the identifiers on retrieval.
TRACKS = {
    350: "05_SS1-68-E_comp",
    352: "05_SS2-107-Ab_comp",
    354: "05_SS2-88-F_comp",
    356: "05_SS3-84-Bb_comp",
    358: "05_SS3-98-C_comp",
}
NOTES = "C C# D D# E F F# G G# A A# B".split()


def prepare(directory, index, track):
    query = urlencode(dict(dataset="jhartquist/guitarset", config="default",
                           split="train", offset=index, length=1))
    with urlopen(f"https://datasets-server.huggingface.co/rows?{query}", timeout=60) as response:
        row = json.load(response)["rows"][0]["row"]
    if row["track_id"] != track:
        raise ValueError(f"Expected {track}, got {row['track_id']}")
    events = []
    for note in row["notes"]:
        midi = int(note["midi"] + 0.5)
        name = f"{NOTES[midi % 12]}{midi // 12 - 1}"
        events.append(dict(time=note["onset_s"], duration=note["offset_s"] - note["onset_s"],
                           events=[dict(note=name)]))
    chart = dict(
        metadata=dict(format_version="1.10.0", author="Xi et al.", license="CC-BY-4.0",
                      source="https://zenodo.org/records/3371780"),
        song=dict(title=track, artist="GuitarSet", genre="Practice", tempo_bpm=row["tempo_bpm"],
                  key=row["key"], difficulty="easy"),
        timing=dict(resolution=480, tempo_map=[dict(tick=0, bpm=row["tempo_bpm"])]),
        instrument=dict(type="guitar", strings=["E4", "B3", "G3", "D3", "A2", "E2"],
                        frets=20, body="acoustic"),
        scoring=dict(perfect_window_ms=50, good_window_ms=100, miss_window_ms=130),
        track=events,
    )
    folder = directory / track
    folder.mkdir(parents=True, exist_ok=True)
    with urlopen(row["audio_mono_mic"][0]["src"], timeout=60) as response:
        subprocess.run(["ffmpeg", "-y", "-loglevel", "error", "-i", "pipe:0", "-ac", "1",
                        "-c:a", "pcm_s16le", str(folder / "recording.wav")],
                       input=response.read(), check=True)
    (folder / "expected.harpchart").write_text(json.dumps(chart), encoding="utf-8")
    print(track, flush=True)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("directory", type=Path)
    args = parser.parse_args()
    for index, track in TRACKS.items():
        prepare(args.directory, index, track)
