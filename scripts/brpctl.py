# SPDX-License-Identifier: MIT

"""Drive a running `--features dev` Harmonicon from the shell, over BRP.

`contributing/src/remote-control.md` documents the protocol and its traps;
this is those request shapes with the fiddly parts already handled — chiefly
resolving a button from its visible label, which means walking `ChildOf`
upward from the text node until an entity in the `Button` set is reached
(`dialogs/button.rs` wraps its content in a shell, so the text is several
levels below the clickable entity).

Standard library only, so it runs with no virtualenv — unlike the asset
tools beside it, this is meant to be reachable mid-debugging.

    python3 scripts/brpctl.py buttons              # what is clickable now
    python3 scripts/brpctl.py click Play
    python3 scripts/brpctl.py click ↻ --in View       # icon-only, disambiguated by its row
    python3 scripts/brpctl.py shot                 # -> target/screenshots/
    python3 scripts/brpctl.py state menu Options
    python3 scripts/brpctl.py resize 1920 1080     # physical px
    python3 scripts/brpctl.py video 300            # -> target/video/NNNN/

    # Getting somewhere, and capturing it under a name you chose
    python3 scripts/brpctl.py home
    python3 scripts/brpctl.py enter "Play 2D" Traditional "Amazing Grace"
    python3 scripts/brpctl.py jam Traditional "Amazing Grace"
    python3 scripts/brpctl.py editor Traditional "Amazing Grace"
    python3 scripts/brpctl.py results        # play the shortest chart to its end
    python3 scripts/brpctl.py pause
    python3 scripts/brpctl.py wait on
    python3 scripts/brpctl.py loop 12 20         # A–B range in seconds; `loop off` clears
    python3 scripts/brpctl.py autoplay on [late_ms]  # play the chart itself (dev build only)
    python3 scripts/brpctl.py capture /tmp/shots before-fix

    # Drive every screen the gameplay work needs a baseline of into one
    # directory of named PNGs. Takes a few minutes.
    python3 scripts/brpctl.py --take-all-screenshots [outdir]

Or import it: `from brpctl import click, shot, texts`.
"""

import glob
import json
import os
import shutil
import sys
import time
import urllib.error
import urllib.request

URL = "http://127.0.0.1:15702"

TEXT = "bevy_ui::widget::text::Text"
# A label can also be assembled from spans — the file dialog's rows are one
# `Text` holding the 📁 icon plus a `TextSpan` child holding the name. Reading
# `Text` alone reports every row as "📁" and nothing else.
TEXT_SPAN = "bevy_text::text::TextSpan"
CHILD_OF = "bevy_ecs::hierarchy::ChildOf"
CHILDREN = "bevy_ecs::hierarchy::Children"
# These two do *not* follow the same rule, and guessing either from the other
# costs an afternoon. The component is registered under its declaring module
# (`…::button::Button`); the `Activate` event is registered under the crate
# root, because `dev_capture` registers it as the re-exported
# `bevy::ui_widgets::Activate`. A wrong component path silently matches
# nothing; a wrong event path at least errors with "Unknown event type".
BUTTON = "bevy_ui_widgets::button::Button"
ACTIVATE = "bevy_ui_widgets::Activate"
WINDOW = "bevy_window::window::Window"
SCREENSHOT = "bevy_render::view::window::screenshot::Screenshot"

# `NextState` lives under `resources`, not `states` — the latter path exists
# and resolves to nothing. Only screens needing no prior selection are
# reachable this way; see `remote-control.md` on why Play/Results are not.
STATE_RESOURCES = {
    "app": "bevy_state::state::resources::NextState<harmonicon_app::app::AppState>",
    "menu": "bevy_state::state::resources::NextState<harmonicon_menu::menu::routing::MenuPage>",
}

_next_id = [0]


def rpc(method, params=None):
    _next_id[0] += 1
    body = {"jsonrpc": "2.0", "id": _next_id[0], "method": method}
    if params is not None:
        body["params"] = params
    request = urllib.request.Request(
        URL,
        data=json.dumps(body).encode(),
        headers={"Content-Type": "application/json"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            payload = json.loads(response.read())
    except urllib.error.URLError as exc:
        raise SystemExit(
            f"no BRP server on {URL} ({exc}) — is the game running with "
            f"--features dev?"
        ) from exc
    if "error" in payload:
        raise RuntimeError(f"{method}: {payload['error']}")
    return payload.get("result")


def query(components=None, with_=None):
    data = {"components": components} if components else {}
    filt = {"with": with_} if with_ else {}
    return rpc("world.query", {"data": data, "filter": filt})


def snapshot():
    """`(labels, parent_of, button_ids)` from three back-to-back queries.

    Kept to as few round trips as possible on purpose: a menu page despawns
    and respawns its whole subtree on navigation, so an entity id read in one
    request can already be gone by the next — which looks exactly like "that
    button doesn't exist".
    """
    labels = [(r["entity"], _unwrap(r["components"][TEXT])) for r in query([TEXT])]
    labels += [(r["entity"], _unwrap(r["components"][TEXT_SPAN])) for r in query([TEXT_SPAN])]
    parent_of = {r["entity"]: _unwrap(r["components"][CHILD_OF]) for r in query([CHILD_OF])}
    button_ids = {r["entity"] for r in query(with_=[BUTTON])}
    return labels, parent_of, button_ids


def _unwrap(value):
    """Newtype components (`Text`, `ChildOf`) serialise as their inner value,
    but a one-element list turns up often enough in BRP payloads to be worth
    tolerating rather than crashing on."""
    return value[0] if isinstance(value, list) and len(value) == 1 else value


def _owning_button(entity, parent_of, button_ids, max_depth=12):
    for _ in range(max_depth):
        if entity in button_ids:
            return entity
        entity = parent_of.get(entity)
        if entity is None:
            return None
    return None


def _subtree_texts(root, children, text_of, max_depth=8):
    found = []
    if text_of.get(root):
        found.append(text_of[root])
    if max_depth:
        for child in children.get(root, ()):
            found += _subtree_texts(child, children, text_of, max_depth - 1)
    return found


def buttons():
    """Every clickable button, as `[(label, context, entity)]`.

    A button's `label` is **every** text node under it, joined — not the first
    one found, or a button whose icon and words are separate nodes would be
    indistinguishable from one that is only an icon.

    `context` is the same for its *parent's* subtree, which is what separates
    two genuinely identically-labelled buttons. The HUD's pause control and
    the pause menu's wait-for-note toggle are both exactly `"⏸"`; only the
    row around them differs (`"Wait for Note: on ⏸"` vs `"⏸"`). Choosing
    between them by position in this list would be choosing by ECS query
    order, i.e. by nothing.

    Beware: this sees the whole ECS tree, so it lists buttons that are
    spawned but **hidden** — the pause menu's own controls are here even
    while the song is playing. BRP has no notion of what is on screen.
    """
    labels, parent_of, button_ids = snapshot()
    text_of = dict(labels)
    # ECS query order is unrelated to visual child order. Read Children so
    # multi-label controls and catalog rows match their on-screen wording.
    children = {r["entity"]: r["components"][CHILDREN] for r in query([CHILDREN])}

    found = []
    for entity in button_ids:
        label = " ".join(_subtree_texts(entity, children, text_of))
        parent = parent_of.get(entity)
        context = (
            " ".join(_subtree_texts(parent, children, text_of)) if parent else label
        )
        found.append((label, context, entity))
    return found


def _matches(value, wanted, exact):
    if wanted is None:
        return True
    return value == wanted if exact else wanted.lower() in value.lower()


def find(label, exact=False, context=None, context_exact=False):
    """Buttons whose label matches — substring and case-insensitive by default.

    `context` further constrains the surrounding row; see [`buttons`] for why
    that is the discriminator rather than an index. It needs `context_exact`
    surprisingly often, because one ambiguous button's context is frequently a
    *substring* of the other's — the HUD's pause row is `"⏸"`, which appears
    inside the pause menu's `"Wait for Note: on ⏸"` too.
    """
    return [
        (text, ctx, entity)
        for text, ctx, entity in buttons()
        if _matches(text, label, exact) and _matches(ctx, context, context_exact)
    ]


def click(label, exact=False, context=None, context_exact=False, index=0):
    """Activate the button labelled `label`, returning the label it matched.

    `Activate` is what a real click and a focused Enter/Space both fire, and
    every click handler in this codebase is an `On<Activate>`, so this reaches
    any of them. It carries its own target, hence the entity in the payload.
    """
    hits = find(label, exact, context, context_exact)
    if not hits:
        available = sorted({text for text, _, _ in buttons() if text})
        raise SystemExit(
            f"no button matching {label!r}"
            + (f" in context {context!r}" if context else "")
            + f". Available: {available}"
        )
    text, _, entity = hits[index]
    rpc("world.trigger_event", {"event": ACTIVATE, "value": {"entity": entity}})
    return text


def texts():
    """Every UI string currently on screen, in query order — `Text` nodes,
    then `TextSpan` children (see `TEXT_SPAN`)."""
    return [_unwrap(r["components"][TEXT]) for r in query([TEXT])] + [
        _unwrap(r["components"][TEXT_SPAN]) for r in query([TEXT_SPAN])
    ]


def shot():
    """Capture the primary window into `target/screenshots/shot_<millis>.png`."""
    return rpc("world.spawn_entity", {"components": {SCREENSHOT: {"Window": "Primary"}}})


def video(frames):
    """Record `frames` rendered frames into a fresh `target/video/NNNN/`.

    One GPU readback per frame stalls the render loop, so the clip runs slower
    than real time — it shows *what* happened, never *how fast*.
    """
    rpc(
        "world.mutate_resources",
        {
            "resource": "harmonicon::dev_capture::VideoCapture",
            "path": ".frames_left",
            "value": int(frames),
        },
    )


def set_state(kind, value):
    rpc(
        "world.mutate_resources",
        {"resource": STATE_RESOURCES[kind], "path": "", "value": {"Pending": value}},
    )


def set_loop(start_secs, end_secs):
    """Set an active A–B loop range directly. The real path is a pointer
    drag on the progress bar while paused, which BRP cannot perform; this
    writes `LoopConfig` whole, skipping `loop_range_valid`, so pass a range
    with `end > start` yourself."""
    rpc(
        "world.mutate_resources",
        {
            "resource": "harmonicon_gameplay::gameplay::state::LoopConfig",
            "path": "",
            "value": {"active": True, "start_time": float(start_secs), "end_time": float(end_secs)},
        },
    )


def set_autoplay(enabled, late_ms=0.0):
    """Have the game sound every note of the chart itself, as if a perfect
    player were on the mic — the only way to get *hits* (head pops, hold
    fills, a populated timing bar on the results screen) out of a headless
    run. `late_ms` plays every attack that much behind the beat, to make the
    results screen's timing lopsided on purpose."""
    rpc(
        "world.mutate_resources",
        {
            "resource": "harmonicon_gameplay::gameplay::autoplay::Autoplay",
            "path": "",
            "value": {"enabled": bool(enabled), "late_ms": float(late_ms)},
        },
    )


def clear_loop():
    rpc(
        "world.mutate_resources",
        {
            "resource": "harmonicon_gameplay::gameplay::state::LoopConfig",
            "path": "",
            "value": {"active": False, "start_time": 0.0, "end_time": 0.0},
        },
    )


def window():
    result = query([WINDOW])
    if not result:
        raise SystemExit("no primary window")
    return result[0]["entity"], result[0]["components"][WINDOW]


def resize(width, height):
    """Resize the window, in **physical** px — `1920, 1080` means Full HD.

    Note that layout code reasons in *logical* px (physical / scale factor),
    so `CompactLayout`'s 900px breakpoint is not 900 here: on a 1.5x display
    a 1920px-wide window is 1280 logical. Ask [`window`] for the live scale
    factor when that distinction matters.
    """
    entity, current = window()
    resolution = current["resolution"]
    resolution["physical_width"] = int(round(width))
    resolution["physical_height"] = int(round(height))
    # Apply both dimensions together: an OS resize event between separate
    # writes can restore the old height while accepting the new width.
    rpc(
        "world.mutate_components",
        {"entity": entity, "component": WINDOW, "path": ".resolution", "value": resolution},
    )


# ── The screenshot tour ───────────────────────────────────────────────────
#
# `--take-all-screenshots` drives the app through every screen the gameplay
# work needs a before/after of (docs/gameplay_improvement_plan.md, Phase 0)
# and writes one named PNG per screen per window size. Named, because
# `shot_<millis>.png` files are only distinguishable by remembering the order
# you took them in.

# Chart fixtures, chosen so the contextual parts of the HUD differ: the
# technique legend only lists what a chart uses, and a non-blues chart must
# show no blues form at all.
FIXTURES = [
    ("simple", "Traditional", "Amazing Grace"),  # no modifiers, no chords
    ("technique", "Example Artist", "Example Song"),  # bend, vibrato, wah
    ("chromatic", "Ludwig van Beethoven", "Fur Elise"),  # chromatic + slide
]

# Physical px. **Full HD is the supported floor** — even phones ship 1080p
# panels now — so there is no "compact desktop" case to baseline. Smaller
# than this is Android-portrait territory, which nobody has run on hardware
# yet (see contributing/src/android-build.md); when that happens it wants its
# own entry here rather than a shrunken desktop one.
SIZES = [("fullhd", 1920, 1080)]

COUNTDOWN_SETTLE = 9.0  # 3s countdown, plus time for notes to reach the lane


def capture(outdir, name):
    """Screenshot, then rename the newest capture to `name.png`.

    The game names its own files, so there's nothing to pass in — take it,
    wait for the readback, then claim whatever landed.
    """
    before = set(glob.glob("target/screenshots/*.png"))
    shot()
    for _ in range(40):
        time.sleep(0.25)
        new = set(glob.glob("target/screenshots/*.png")) - before
        if new:
            os.makedirs(outdir, exist_ok=True)
            target = os.path.join(outdir, f"{name}.png")
            shutil.copy(new.pop(), target)
            print(f"  {target}")
            return target
    raise SystemExit(f"no screenshot appeared for {name!r}")


def _go(label, settle=2.0, exact=False, context=None):
    click(label, exact=exact, context=context)
    time.sleep(settle)


# The HUD's pause control is exactly "⏸"; the pause menu's wait toggle used
# to read the same until `TextSpan` labels were included, and now reads
# "⏸  Wait for Note". `context_exact` still guards the pause control against
# any future icon-only sibling.
def _pause(settle=2.0):
    click("⏸", exact=True, context="⏸", context_exact=True)
    time.sleep(settle)


def _set_wait(on, settle=2.0):
    """Put wait-for-note into a known state, rather than toggling it.

    It is a persisted setting, so a blind toggle does the opposite of what the
    tour wants whenever the previous session left it on — which produced a
    "wait-for-note" capture of ordinary play, and left every later fixture
    frozen at the hit line.
    """
    # The button reads "⏸  Wait for Note" (icon `Text` + name `TextSpan`);
    # its row also carries the "Wait for Note: on/off" readout.
    hits = find("Wait for Note", context="Wait for Note:")
    if not hits:
        raise SystemExit("the wait-for-note toggle is only in the pause menu")
    _, context, _ = hits[0]
    if ("Wait for Note: on" in context) == bool(on):
        return
    click("Wait for Note", context="Wait for Note:")
    time.sleep(settle)


def to_main_menu():
    """Get back to a known screen from wherever the app currently is.

    Via state rather than by clicking back arrows: a song can reach its end
    and move to `Results` on its own schedule, so there is no reliable count
    of "←" presses to get home from.
    """
    set_state("app", "Menu")
    time.sleep(1.0)
    set_state("menu", "Main")
    time.sleep(1.5)


def enter_song(mode, artist, song):
    """Main menu → unified picker → harp check → a playing song."""
    _go("Play", exact=True)
    _go("Play Song")
    # Play Song opens in 2D; the single switch flips to 3D when requested.
    if mode == "Play 3D":
        _go("2d 3d", exact=True)
    elif mode != "Play 2D":
        raise ValueError(f"unknown gameplay mode: {mode!r}")
    _go(f"{song} {artist}")
    _go("Play", exact=True, settle=COUNTDOWN_SETTLE)


def enter_jam(artist, song):
    """Main menu → a Jam Session on a real song. Play → Jam Session → Pick a
    Song, then the same unified picker → harp-check tail as `enter_song`."""
    _go("Play", exact=True)
    _go("Jam Session")
    _go("Pick a Song")
    _go(f"{song} {artist}")
    _go("Play", exact=True, settle=COUNTDOWN_SETTLE)


def enter_editor(artist, song, chart="chart.harpchart"):
    """Main menu → the Song Editor with a bundled chart loaded, via its own
    file dialog: 📂 → artist folder → song folder → `song/` → the chart.

    The dialog's rows are a 📁 `Text` plus a `TextSpan` holding the name, so
    they match on the name (see `TEXT_SPAN`)."""
    set_state("app", "SongEditor2")
    time.sleep(2.0)
    _go("📂", exact=True)
    _go(artist)
    _go(song)
    # Exact: a bare "song" also matches the editor's "‹ Record Song ›"
    # toggle, which switched it to lesson mode and failed the load.
    _go("📁  song", exact=True)
    _go(chart, settle=3.0)


def wait_for_results(timeout_secs=180.0):
    """Block until the song currently playing ends and the results screen is
    up — recognised by its Retry button, which exists nowhere else.

    Results can't be reached by state: it needs a finished run, and the
    shortest bundled chart is the quickest honest way to one.
    """
    deadline = time.monotonic() + timeout_secs
    while time.monotonic() < deadline:
        if any("Retry" in label for label, _, _ in buttons()):
            return True
        time.sleep(2.0)
    return False


# The shortest bundled chart: what to play when only the *end* matters.
SHORTEST_SONG = ("Example Artist", "Example Song 3")


def take_all_screenshots(outdir="target/screenshots/tour"):
    for size_name, width, height in SIZES:
        resize(width, height)
        time.sleep(2.0)
        print(f"[{size_name}] {width}x{height} physical")

        for fixture, artist, song in FIXTURES:
            to_main_menu()
            enter_song("Play 2D", artist, song)
            capture(outdir, f"play2d-{fixture}-{size_name}")

            # Only the first fixture needs the transient states; they are a
            # property of the HUD, not of the chart.
            if fixture == FIXTURES[0][0]:
                _pause()
                capture(outdir, f"play2d-paused-{size_name}")
                # Wait-for-note is reached the way a player reaches it: from
                # the pause menu, then back to the song, where it freezes at
                # the first note that comes due.
                _set_wait(True)
                _go("Resume", settle=4.0)
                capture(outdir, f"play2d-wait-for-note-{size_name}")
                _pause()
                _set_wait(False)  # every later fixture wants ordinary play
                _go("Resume")
                set_loop(12, 24)
                time.sleep(1.0)
                capture(outdir, f"play2d-loop-{size_name}")
                clear_loop()

        to_main_menu()
        enter_song("Play 3D", *FIXTURES[0][1:])
        capture(outdir, f"play3d-{FIXTURES[0][0]}-{size_name}")

        to_main_menu()
        enter_song("Play 2D", *SHORTEST_SONG)
        print("  waiting for the song to end…")
        wait_for_results()
        capture(outdir, f"results-{size_name}")

    to_main_menu()
    print(f"\nWrote the tour to {outdir}/")


def _main(argv):
    if not argv:
        raise SystemExit(__doc__)
    command, args = argv[0], argv[1:]
    if command == "buttons":
        for text, context, entity in sorted(buttons()):
            if text:
                print(f"{entity}\t{text!r}\tin {context!r}")
    elif command == "texts":
        for text in texts():
            print(repr(text))
    elif command == "click":
        # `click Label` or `click Label --in "row text"`, for the icon-only
        # buttons that share a glyph and differ only by the row they sit in.
        context = None
        if "--in" in args:
            i = args.index("--in")
            context = " ".join(args[i + 1 :])
            args = args[:i]
        print(click(" ".join(args), context=context))
    elif command == "shot":
        print(shot())
    elif command == "video":
        video(args[0])
    elif command == "state":
        set_state(args[0], args[1])
    elif command == "resize":
        resize(float(args[0]), float(args[1]))
    elif command == "sleep":
        time.sleep(float(args[0]))
    elif command == "capture":
        capture(args[0], args[1])
    elif command == "home":
        to_main_menu()
    elif command == "enter":
        enter_song(args[0], args[1], " ".join(args[2:]))
    elif command == "jam":
        enter_jam(args[0], " ".join(args[1:]))
    elif command == "editor":
        enter_editor(args[0], " ".join(args[1:]))
    elif command == "results":
        enter_song("Play 2D", *SHORTEST_SONG)
        print("finished" if wait_for_results() else "timed out waiting for the song to end")
    elif command == "pause":
        _pause()
    elif command == "wait":
        _set_wait(args[0].lower() in ("on", "true", "1"))
    elif command == "loop":
        if args and args[0].lower() in ("off", "clear"):
            clear_loop()
        else:
            set_loop(args[0], args[1])
    elif command == "autoplay":
        on = args[0].lower() in ("on", "true", "1")
        set_autoplay(on, args[1] if len(args) > 1 else 0.0)
    elif command in ("--take-all-screenshots", "take-all-screenshots"):
        take_all_screenshots(*args[:1])
    else:
        raise SystemExit(f"unknown command {command!r}\n{__doc__}")


if __name__ == "__main__":
    _main(sys.argv[1:])
