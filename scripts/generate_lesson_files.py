# SPDX-License-Identifier: MIT

import json
import os

ROOT = "assets/lessons"
BLOW = ["C4","E4","G4","C5","E5","G5","C6","E6","G6","C7"]
DRAW = ["D4","G4","B4","D5","F5","A5","B5","D6","F6","A6"]

def harmonica(key="C", position="1st"):
    return {
        "type": "diatonic",
        "holes": 10,
        "position": position,
        "bending_profile": "richter_standard",
        "layout": {"blow": BLOW, "draw": DRAW},
    }

def r3(x):
    return round(x, 3)

def base_chart(title, description, tempo_bpm, key, difficulty, position, track,
                scoring=None, feel=None):
    song = {
        "title": title,
        "artist": "Harmonicon Lessons",
        "tempo_bpm": tempo_bpm,
        "key": key,
        "time_signature": "4/4",
        "difficulty": difficulty,
    }
    if feel:
        song["feel"] = feel
    chart = {
        "metadata": {
            "format_version": "1.0.0",
            "author": "Harmonicon",
            "source": "Original exercise",
            "license": "MIT",
            "description": description,
        },
        "song": song,
        "timing": {
            "resolution": 480,
            "tempo_map": [{"tick": 0, "bpm": tempo_bpm}],
        },
        "harmonica": harmonica(key, position),
        "track": track,
        "scoring": scoring or {
            "perfect_window_ms": 150,
            "good_window_ms": 350,
            "miss_window_ms": 550,
        },
    }
    return chart

def note_item(item_id, time, duration, hole, action, note, phrase=None, modifiers=None):
    ev = {"hole": hole, "action": action, "note": note}
    if modifiers:
        ev["modifiers"] = modifiers
    item = {
        "id": item_id,
        "time": r3(time),
        "duration": r3(duration),
        "play_mode": "single",
        "events": [ev],
    }
    if phrase:
        item["phrase"] = phrase
    return item

def write_chart(path, chart):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(chart, f, indent=2)
        f.write("\n")
    print("wrote", path)

# ── 1. breathing ──────────────────────────────────────────────────────────
track = []
holes = [(1, "blow", "C4"), (1, "draw", "D4"), (2, "blow", "E4"), (2, "draw", "G4"),
         (3, "blow", "G4"), (3, "draw", "B4"), (4, "blow", "C5"), (4, "draw", "D5")]
t = 0.0
for i, (hole, action, note) in enumerate(holes):
    phrase = f"Hole {hole}" if action == "blow" else None
    track.append(note_item(f"n{i+1:02}", t, 3.5, hole, action, note, phrase))
    t += 4.0
chart = base_chart(
    "Breathing: Long Tones",
    "Lesson drill: diaphragm breathing and long tones on holes 1-4 blow/draw, held "
    "3-4 beats at a slow tempo. Original technique exercise, no melody.",
    60, "C", "easy", "1st", track,
)
write_chart(f"{ROOT}/01_blowing/07_breathing/song/chart.harpchart", chart)

# ── 2. first-bend ─────────────────────────────────────────────────────────
track = []
t = 0.0
for i in range(6):
    bent = i % 2 == 1
    mods = [{"type": "bend", "semitones": -1}] if bent else None
    phrase = "4 Draw" if i == 0 else ("4 Draw bent" if i == 1 else None)
    track.append(note_item(f"n{i+1:02}", t, 3.5, 4, "draw", "D5", phrase, mods))
    t += 4.0
chart = base_chart(
    "First Bend: 4 Draw",
    "Lesson drill: the 4-draw half-step bend, alternating plain 4 draw and 4 draw "
    "bent a half step. Original technique exercise, no melody.",
    60, "C", "easy", "2nd", track,
)
write_chart(f"{ROOT}/01_blowing/08_first_bend/song/chart.harpchart", chart)

# ── 3. deep-bends ─────────────────────────────────────────────────────────
track = []
seq = [
    (2, "draw", "G4", -1, "2 Draw half-step"),
    (2, "draw", "G4", -2, "2 Draw whole-step"),
    (3, "draw", "B4", -1, "3 Draw half-step"),
    (3, "draw", "B4", -2, "3 Draw whole-step"),
]
t = 0.0
for rep in range(2):
    for i, (hole, action, note, semis, phrase) in enumerate(seq):
        track.append(note_item(
            f"n{rep*4+i+1:02}", t, 3.5, hole, action, note,
            phrase if rep == 0 else None,
            [{"type": "bend", "semitones": semis}],
        ))
        t += 4.0
chart = base_chart(
    "Deep Bends: 2 and 3 Draw",
    "Lesson drill: the 2-draw half/whole-step bend and the 3-draw half/whole-step "
    "bend — the notes 2nd-position blues lives on. Original technique exercise, no "
    "melody.",
    60, "C", "intermediate", "2nd", track,
)
write_chart(f"{ROOT}/01_blowing/09_deep_bends/song/chart.harpchart", chart)

# ── 4. vibrato ────────────────────────────────────────────────────────────
track = []
seq = [(4, "blow", "C5", "Blow 4"), (4, "draw", "D5", "Draw 4"),
       (5, "blow", "E5", "Blow 5"), (5, "draw", "F5", "Draw 5")]
t = 0.0
for i, (hole, action, note, phrase) in enumerate(seq):
    track.append(note_item(
        f"n{i+1:02}", t, 5.0, hole, action, note, phrase,
        [{"type": "vibrato", "oscillation_hz": 4.5}],
    ))
    t += 6.0
chart = base_chart(
    "Vibrato: Held Middle Notes",
    "Lesson drill: throat/diaphragm vibrato on held middle-register notes, around "
    "4-5 Hz. Original technique exercise, no melody.",
    60, "C", "intermediate", "1st", track,
)
write_chart(f"{ROOT}/01_blowing/10_vibrato/song/chart.harpchart", chart)

# ── 5. articulation ───────────────────────────────────────────────────────
track = []
seq = [(4, "blow", "C5", "Ta-ka on Blow 4"), (4, "draw", "D5", "Ta-ka on Draw 4"),
       (5, "blow", "E5", "Ta-ka on Blow 5")]
tempo = 100
eighth = 60.0 / tempo / 2.0
t = 0.0
idx = 1
for hole, action, note, phrase in seq:
    for j in range(8):
        track.append(note_item(
            f"n{idx:02}", t, eighth * 0.9, hole, action, note,
            phrase if j == 0 else None,
        ))
        t += eighth
        idx += 1
    t += 0.5  # a breath between hole groups
chart = base_chart(
    "Articulation: Ta-Ka Tonguing",
    "Lesson drill: repeated eighth notes on one hole at a time — re-articulate "
    "every note (\"ta-ka\" tonguing) rather than holding a slurred tone. Original "
    "technique exercise, no melody.",
    tempo, "C", "intermediate", "1st", track,
)
write_chart(f"{ROOT}/01_blowing/11_articulation/song/chart.harpchart", chart)

# ── 6. counting-four ──────────────────────────────────────────────────────
track = []
tempo = 80
beat = 60.0 / tempo
idx = 1
t = 0.0
# Section 1: every beat, 4 bars (16 quarter notes).
for i in range(16):
    phrase = "Count every beat" if i == 0 else None
    track.append(note_item(f"n{idx:02}", t, beat * 0.85, 4, "blow", "C5", phrase))
    t += beat
    idx += 1
# Section 2: beats 1 and 3 only, 4 bars.
bar = beat * 4
for b in range(4):
    bar_start = t + b * bar
    for beat_ix, phrase in [(0, "Beats 1 and 3" if b == 0 else None), (2, None)]:
        track.append(note_item(
            f"n{idx:02}", bar_start + beat_ix * beat, beat * 0.85, 4, "blow", "C5", phrase,
        ))
        idx += 1
t += 4 * bar
# Section 3: beat 1 only, 4 bars.
for b in range(4):
    phrase = "Beat 1 only" if b == 0 else None
    track.append(note_item(f"n{idx:02}", t + b * bar, beat * 0.85, 4, "blow", "C5", phrase))
    idx += 1
chart = base_chart(
    "Counting Four",
    "Lesson drill: count 1-2-3-4 aloud while playing — quarter notes on every "
    "beat, then beats 1 and 3 only, then beat 1 only, with the metronome "
    "prominent throughout. Original rhythm exercise, no melody.",
    tempo, "C", "easy", "1st", track,
)
write_chart(f"{ROOT}/02_rhythm/05_counting_four/song/chart.harpchart", chart)

# ── 7. bar-counting ───────────────────────────────────────────────────────
track = []
tempo = 80
beat = 60.0 / tempo
bar = beat * 4
# Standard progression bar roots: I I I I IV IV I I V IV I V, 2nd position
# (song key G on a C-richter harp): I=2 draw (G4), IV=4 blow (C5), V=4 draw (D5).
roots = [
    (2, "draw", "G4"), (2, "draw", "G4"), (2, "draw", "G4"), (2, "draw", "G4"),
    (4, "blow", "C5"), (4, "blow", "C5"),
    (2, "draw", "G4"), (2, "draw", "G4"),
    (4, "draw", "D5"),
    (4, "blow", "C5"),
    (2, "draw", "G4"),
    (4, "draw", "D5"),
]
for i, (hole, action, note) in enumerate(roots):
    phrase = "Bar 1 (I)" if i == 0 else ("Bar 5 (IV)" if i == 4 else ("Bar 9 (V)" if i == 8 else None))
    track.append(note_item(f"n{i+1:02}", i * bar, beat * 0.7, hole, action, note, phrase))
chart = base_chart(
    "Counting the Bars",
    "Lesson drill: through a full 12-bar cycle, play only the chord root on beat "
    "1 of each bar — 2 draw over the I chord, 4 blow over the IV chord, 4 draw "
    "over the V chord (2nd position). Teaches bar counting and hearing the "
    "changes at once. Original rhythm exercise, no melody.",
    tempo, "G", "intermediate", "2nd", track,
)
write_chart(f"{ROOT}/02_rhythm/06_bar_counting/song/chart.harpchart", chart)

# ── 8. turnaround ─────────────────────────────────────────────────────────
tempo = 80
beat = 60.0 / tempo
bar = beat * 4
track = [
    note_item("n01", 11 * bar, 2.0, 4, "draw", "D5", "Bar 12 (V) — the turnaround"),
    note_item("n02", 12 * bar, 2.0, 2, "draw", "G4", "Bar 1 (I) — resolved"),
]
chart = base_chart(
    "The Turnaround",
    "Lesson drill: feel bars 11-12 of the 12-bar form — this chart rests through "
    "the rest of the form entirely, then lands the V-chord root in bar 12 and "
    "resolves to the I-chord root at the top of the next chorus. Lose count and "
    "you'll play into silence and miss the landing. Original rhythm exercise, no "
    "melody.",
    tempo, "G", "intermediate", "2nd", track,
)
write_chart(f"{ROOT}/02_rhythm/07_turnaround/song/chart.harpchart", chart)

# ── 9. shuffle-feel ───────────────────────────────────────────────────────
tempo = 90
beat = 60.0 / tempo
long_len = beat * 2.0 / 3.0
short_len = beat * 1.0 / 3.0
track = []
idx = 1
for i in range(8):
    t0 = i * beat
    track.append(note_item(
        f"n{idx:02}", t0, long_len * 0.85, 4, "blow", "C5",
        "Swung pairs on hole 4" if i == 0 else None,
    ))
    idx += 1
    track.append(note_item(f"n{idx:02}", t0 + long_len, short_len * 0.85, 4, "draw", "D5"))
    idx += 1
chart = base_chart(
    "Shuffle Feel",
    "Lesson drill: straight vs. swung eighths — swung long-short pairs on hole 4, "
    "declared feel: shuffle so the metronome swings with it. Original rhythm "
    "exercise, no melody.",
    tempo, "C", "easy", "1st", track, feel="shuffle",
)
write_chart(f"{ROOT}/02_rhythm/08_shuffle_feel/song/chart.harpchart", chart)

print("all charts written")
