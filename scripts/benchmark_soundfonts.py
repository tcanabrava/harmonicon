# SPDX-License-Identifier: MIT
# /// script
# dependencies = ["numpy>=2", "pyfluidsynth==1.4.0"]
# ///
"""Render reference presets, extract timbre templates and measure exact note sets."""

import argparse
import json
import re
import wave
from dataclasses import dataclass
from pathlib import Path

import fluidsynth
import numpy as np

ROOT = Path(__file__).resolve().parents[1]
BANKS = ROOT / "assets/soundfonts"
GENERAL = "GeneralUser-GS.sf2"
RATE = 44_100
WINDOW = 4096
HOP = 2048
HANN = np.hanning(WINDOW)
NAMES = "C C# D D# E F F# G G# A A# B".split()


@dataclass(frozen=True)
class Preset:
    program: int
    name: str
    low: int
    high: int
    # Sounding MIDI pitches, including real chord voicings where applicable.
    examples: tuple
    chart: dict | None
    bank_file: str = GENERAL


GUITAR = dict(type="guitar", strings="E4 B3 G3 D3 A2 E2".split())
GUITAR_EXAMPLES = ((40,), (59,), (76,), (40, 47, 52, 55, 59, 64), (48, 55, 60, 64))
PRESETS = {
    "harmonica": Preset(22, "Harmonica", 59, 99,
                        ((60,), (72,), (91,), (60, 64, 67), (60, 72), (72, 76, 79, 84)), None),
    "acoustic-guitar": Preset(25, "Steel Guitar", 40, 84, GUITAR_EXAMPLES,
                              dict(GUITAR, body="acoustic", frets=20)),
    "electric-guitar": Preset(27, "Clean Guitar", 40, 86, GUITAR_EXAMPLES,
                              dict(GUITAR, body="electric", frets=22)),
    "bass": Preset(33, "Finger Bass", 28, 67, ((28,), (43,), (55,), (40, 47), (43, 55)),
                   dict(type="bass", strings="G2 D2 A1 E1".split(), frets=24)),
    "cavaquinho": Preset(0, "Cavaco", 62, 93,
                         ((62,), (71,), (83,), (62, 67, 71, 74), (64, 67, 72, 76)),
                         dict(type="cavaquinho", strings="D5 B4 G4 D4".split(), frets=19),
                         "Cavaco.sf2"),
    "piano": Preset(0, "Grand Piano", 21, 108,
                    ((36,), (60,), (96,), (48, 55, 60, 64, 67), (60, 64, 67), (60, 72)),
                    dict(type="piano", keys=88)),
    "flute": Preset(73, "Flute", 60, 96, ((60,), (72,), (84,), (91,)), dict(type="flute")),
    "clarinet": Preset(71, "Clarinet", 50, 82, ((50,), (62,), (70,), (81,)), dict(type="clarinet")),
    "violin": Preset(40, "Violin", 55, 100, ((55,), (69,), (88,), (55, 62), (69, 76)),
                    dict(type="violin")),
    "viola": Preset(41, "Viola", 48, 93, ((48,), (62,), (81,), (48, 55), (62, 69)),
                   dict(type="viola")),
}


def render(synth, pitches, velocities, length):
    synth.all_sounds_off(0)
    for pitch, velocity in zip(pitches, velocities, strict=True):
        if synth.noteon(0, pitch, velocity) != 0:
            raise ValueError(f"Could not play MIDI {pitch}")
    return synth.get_samples(length).reshape(-1, 2).mean(axis=1) / 32768


def spectra(frames):
    return np.abs(np.fft.rfft(frames * HANN))[:, :WINDOW // 2]


def template_banks(synth, preset):
    generic, measured = [], []
    for midi in range(preset.low, preset.high + 1):
        frequency = 440 * 2 ** ((midi - 69) / 12)
        centers = np.arange(1, 9) * frequency * WINDOW / RATE
        bins = np.arange(WINDOW // 2)
        shapes = np.exp(-0.5 * (bins - centers[:, None]) ** 2)
        # At the top of a piano, stretch tuning can exceed two FFT bins.
        stretch_cents = 40 if preset.chart and preset.chart["type"] == "piano" else 0
        radii = np.maximum(2, np.ceil(centers * (2 ** (stretch_cents / 1200) - 1))).astype(int)
        profiles = []
        for velocity in (52, 80, 108):
            samples = render(synth, [midi], [velocity], round(0.42 * RATE))
            # Begin after the attack, average overlapping sustain spectra.
            sustain = samples[round(0.12 * RATE):]
            frames = np.lib.stride_tricks.sliding_window_view(sustain, WINDOW)[::1024]
            spectrum = spectra(frames).mean(axis=0)
            peaks = np.array([spectrum[max(0, round(c) - r):round(c) + r + 1].max(initial=0)
                              for c, r in zip(centers, radii, strict=True)])
            if peaks.max() < 0.01:
                raise ValueError(f"{preset.name} has no usable sample at MIDI {midi}, v{velocity}")
            profiles.append(peaks / peaks.max())
        for weights, bank in [(1 / np.arange(1, 9), generic), (np.mean(profiles, axis=0), measured)]:
            row = weights @ shapes
            bank.append(row / np.linalg.norm(row))
    return np.array(generic), np.array(measured)


def exercise(synth, preset):
    parts, events = [], []
    # Each event lasts half a second with a quarter-second gap. Test velocities
    # differ from those used for learning templates. Accents alternate in chords.
    for velocity in (64, 96):
        for pitches in preset.examples:
            start = len(parts) * 0.75
            velocities = [velocity + 5 * (i % 2) for i in range(len(pitches))]
            samples = render(synth, pitches, velocities, RATE // 2)
            events.append((start, start + 0.5, pitches))
            for pitch in pitches:
                synth.noteoff(0, pitch)
            release = synth.get_samples(RATE // 4).reshape(-1, 2).mean(axis=1) / 32768
            parts.append(np.concatenate([samples, release]))
    return np.concatenate(parts), events


def score(bank, frames, expected):
    observed = spectra(frames).T
    gram = bank @ bank.T
    target = bank @ observed
    activation = np.ones_like(target)
    for _ in range(50):
        activation *= target / (gram @ activation + 1e-9)
    top = np.argpartition(activation, -6, axis=0)[-6:]
    heard = np.zeros_like(expected)
    np.put_along_axis(heard, top, True, axis=0)
    heard &= (activation > 0) & (activation >= 0.22 * activation.max(axis=0))
    heard &= np.mean(frames ** 2, axis=1) >= 0.005 ** 2
    exact = (heard == expected).all(axis=0)
    counts = expected.sum(axis=0)
    return [int(np.sum(exact & (counts == 1))), int(np.sum(exact & (counts >= 2)))]


def evaluate(samples, events, preset, banks):
    frames = np.lib.stride_tricks.sliding_window_view(samples, WINDOW)[::HOP]
    times = (np.arange(len(frames)) * HOP + WINDOW) / RATE
    expected = np.zeros((preset.high - preset.low + 1, len(frames)), dtype=bool)
    for start, end, pitches in events:
        for pitch in pitches:
            if not preset.low <= pitch <= preset.high:
                raise ValueError(f"MIDI {pitch} outside {preset.name} evaluation range")
            expected[pitch - preset.low] |= (start <= times) & (times < end)
    counts = expected.sum(axis=0)
    return np.array([score(bank, frames, expected) for bank in banks] +
                    [[np.sum(counts == 1), np.sum(counts >= 2)]])


def write_fixture(directory, key, preset, samples, events):
    folder = directory / key
    folder.mkdir(parents=True, exist_ok=True)
    source = ROOT / "assets/debug_songs/synthetic_c_chords/expected.harpchart"
    chart = json.loads(source.read_text())
    chart["song"].update(title=f"SoundFont {preset.name}", artist="Harmonicon", genre="Practice")
    chart["metadata"] = dict(format_version="1.15.0", author="Harmonicon")
    if preset.chart:
        del chart["harmonica"]
        chart["instrument"] = preset.chart
    chart["track"] = []
    for start, end, pitches in events:
        notes = [dict(note=f"{NAMES[p % 12]}{p // 12 - 1}") for p in pitches]
        if key == "harmonica":
            for note in notes:
                note.update(action="blow", hole=1 + chart["harmonica"]["layout"]["blow"].index(note["note"]))
        chart["track"].append(dict(time=start, duration=end - start, events=notes,
                                   play_mode="chord" if len(notes) > 1 else "single"))
    with wave.open(str(folder / "recording.wav"), "wb") as wav:
        wav.setparams((1, 2, RATE, 0, "NONE", ""))
        wav.writeframes(np.clip(np.rint(samples * 32768), -32768, 32767).astype("<i2").tobytes())
    (folder / "expected.harpchart").write_text(json.dumps(chart, indent=2) + "\n")


def load_recording(folder):
    with wave.open(str(folder / "recording.wav"), "rb") as wav:
        if (wav.getnchannels(), wav.getsampwidth(), wav.getframerate()) != (1, 2, RATE):
            raise ValueError("Expected a 44.1 kHz mono PCM16 recording")
        samples = np.frombuffer(wav.readframes(wav.getnframes()), "<i2") / 32768
    events = []
    for item in json.loads((folder / "expected.harpchart").read_text())["track"]:
        pitches = []
        for event in item["events"]:
            pitch, octave = re.fullmatch(r"([A-G]#?)(-?\d+)", event["note"]).groups()
            pitches.append(12 * (int(octave) + 1) + NAMES.index(pitch))
        start = item["time"]
        events.append((start, start + item["duration"], pitches))
    return samples, events


def report(key, scores):
    generic, measured, totals = scores
    print(f"{key:18} " + "  ".join(f"{g:4} {m:4} / {n:4}" for g, m, n in
                                    zip(generic, measured, totals, strict=True)), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, help="also write WAV/chart pairs for note_bench")
    parser.add_argument("--recordings", nargs=2, action="append", default=[],
                        metavar=("PRESET", "DIR"), help="score external 44.1 kHz fixtures")
    args = parser.parse_args()
    for key, location in args.recordings:
        if key not in PRESETS or not list(Path(location).glob("**/expected.harpchart")):
            parser.error(f"No preset or fixtures for {key} {location}")
    synth = fluidsynth.Synth(gain=0.8, channels=16, samplerate=RATE,
                             **{"synth.reverb.active": 0, "synth.chorus.active": 0})
    print("                      Single frames        Chord frames\n"
          "                     base  SF2 / total    base  SF2 / total")
    try:
        fonts = {name: synth.sfload(str(BANKS / name), update_midi_preset=0)
                 for name in sorted({p.bank_file for p in PRESETS.values()})}
        for key, preset in PRESETS.items():
            font = fonts[preset.bank_file]
            if font < 0 or synth.sfpreset_name(font, 0, preset.program) != preset.name:
                raise ValueError(f"Missing {preset.name} in {preset.bank_file}")
            if synth.program_select(0, font, 0, preset.program) != 0:
                raise ValueError(f"Could not select {preset.name}")
            banks = template_banks(synth, preset)
            samples, events = exercise(synth, preset)
            if args.fixtures:
                write_fixture(args.fixtures, key, preset, samples, events)
            report(key, evaluate(samples, events, preset, banks))
            for name, location in args.recordings:
                if key == name:
                    scores = [evaluate(*load_recording(path.parent), preset, banks)
                              for path in sorted(Path(location).glob("**/expected.harpchart"))]
                    report(key + " external", np.sum(scores, axis=0))
    finally:
        synth.delete()


if __name__ == "__main__":
    main()
