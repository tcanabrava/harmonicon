# SPDX-License-Identifier: MIT
# /// script
# dependencies = ["mido==1.3.3"]
# ///
"""Prepare the three MAESTRO v3.0.0 piano excerpts used in the chord study.

Pass the extracted maestro-v3.0.0 directory and a separate output directory.
Requires ffmpeg. Replay the keys and pedal directories separately with note_bench.
"""

import argparse
import csv
import json
import shutil
import subprocess
from pathlib import Path

import mido

SECONDS = 30
NOTES = "C C# D D# E F F# G G# A A# B".split()


def note_events(midi, sustain):
    starts = {}
    released = set()
    pedal_down = False
    events = []

    def finish(pitch, end):
        start = starts.pop(pitch)
        released.discard(pitch)
        if end > start:
            name = f"{NOTES[pitch % 12]}{pitch // 12 - 1}"
            events.append(dict(time=start, duration=end - start, events=[dict(note=name)]))

    time = 0.0
    for message in midi:
        time += message.time  # Mido merges tracks and applies the tempo map.
        if time >= SECONDS:
            break
        if message.type == "note_on" and message.velocity:
            if message.note in starts:
                finish(message.note, time)
            starts[message.note] = time
        elif message.type in ("note_on", "note_off") and message.note in starts:
            if sustain and pedal_down:
                released.add(message.note)
            else:
                finish(message.note, time)
        elif message.type == "control_change" and message.control == 64:
            pedal_down = message.value >= 64
            if not pedal_down:
                for pitch in sorted(released):
                    finish(pitch, time)
    for pitch in list(starts):
        finish(pitch, SECONDS)
    return events


def prepare(dataset, output):
    with (dataset / "maestro-v3.0.0.csv").open(encoding="utf-8") as metadata:
        test = [row for row in csv.DictReader(metadata) if row["split"] == "test"]
    for row in (test[0], test[len(test) // 2], test[-1]):
        tag = f"{row['year']}-{row['canonical_composer'].split()[-1]}"
        midi = mido.MidiFile(dataset / row["midi_filename"])
        for policy in ("keys", "pedal"):
            folder = output / policy / tag
            folder.mkdir(parents=True, exist_ok=True)
            wav = folder / "recording.wav"
            if policy == "keys":
                subprocess.run(["ffmpeg", "-v", "error", "-y", "-i", str(dataset / row["audio_filename"]),
                                "-t", str(SECONDS), "-ac", "1", "-ar", "44100", "-c:a", "pcm_s16le",
                                str(wav)], check=True)
            else:
                shutil.copyfile(output / "keys" / tag / wav.name, wav)
            chart = dict(
                metadata=dict(format_version="1.10.0", author="MAESTRO", license="CC BY-NC-SA 4.0",
                              source="https://magenta.tensorflow.org/datasets/maestro"),
                song=dict(title=row["canonical_title"], artist=row["canonical_composer"],
                          genre="Classical", tempo_bpm=120, key="C", difficulty="easy"),
                timing=dict(resolution=480, tempo_map=[dict(tick=0, bpm=120)]),
                instrument=dict(type="piano", keys=88),
                scoring=dict(perfect_window_ms=50, good_window_ms=100, miss_window_ms=130),
                track=note_events(midi, sustain=policy == "pedal"),
            )
            (folder / "expected.harpchart").write_text(json.dumps(chart) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dataset", type=Path)
    parser.add_argument("output", type=Path)
    args = parser.parse_args()
    prepare(args.dataset, args.output)
