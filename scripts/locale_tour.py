#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Capture every menu page plus the gameplay/results screens, for a
localization-expansion audit: run the game once per locale and compare.

    LANG=pt_BR.UTF-8 ./scripts/run-dev.sh --no-build
    python3 scripts/locale_tour.py target/screenshots/pt-BR

The game follows the OS locale (`localization::system_language`), so the
language is chosen by the environment the binary is launched with, not
over BRP. Pages that need a prior selection (the song list, the harp
check, the lesson reader) are reached by clicking, like `brpctl.py` does;
everything else is reached by state. The whole run takes about a minute.
"""

import os
import re
import sys
import time

sys.path.insert(0, __file__.rsplit("/", 1)[0])
import brpctl  # noqa: E402

ROOT = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..")


def locale_from_env():
    """`pt_BR.UTF-8` → `pt-BR`; anything unrecognised → `en-US`, which is
    also what the game falls back to."""
    lang = os.environ.get("LC_ALL") or os.environ.get("LANG") or ""
    m = re.match(r"([a-z]{2})_([A-Z]{2})", lang)
    return f"{m.group(1)}-{m.group(2)}" if m else "en-US"


def strings(locale):
    """Every simple `key = value` line of a locale's `ui.ftl`, so the tour can
    click the *localized* label for a key. Multi-line/attribute messages
    aren't needed for navigation."""
    path = os.path.join(ROOT, "assets", "locales", locale, "main", "ui.ftl")
    out = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            m = re.match(r"([a-z0-9-]+) = (.+)$", line.rstrip("\n"))
            if m:
                out[m.group(1)] = m.group(2)
    return out


T = strings(locale_from_env())


def go(key, **kw):
    brpctl._go(T[key], **kw)


def enter_song(artist, song):
    go("menu-play", exact=True)
    go("play-song")
    brpctl._go(artist)
    brpctl._go(song)
    go("harp-check-play", exact=True, settle=brpctl.COUNTDOWN_SETTLE)

# Reached by `NextState<MenuPage>` alone.
STATE_PAGES = [
    "Main",
    "Welcome",
    "Play",
    "ArtistList",
    "Options",
    "Theme",
    "LessonTree",
    "JamSessionMenu",
    "JamGenerate",
    "HelpAbout",
]


def wait_for_results(timeout_secs=180.0):
    """`brpctl.wait_for_results` looks for the English Retry button."""
    retry = T["results-retry"]
    deadline = time.monotonic() + timeout_secs
    while time.monotonic() < deadline:
        if any(retry in label for label, _, _ in brpctl.buttons()):
            return True
        time.sleep(2.0)
    return False


def main(outdir):
    brpctl.resize(1920, 1080)
    time.sleep(1.0)
    for page in STATE_PAGES:
        brpctl.to_main_menu()
        brpctl.set_state("menu", page)
        time.sleep(2.0)
        brpctl.capture(outdir, f"menu-{page}")

    # Song list + harp check: click through, since both need a selection.
    brpctl.to_main_menu()
    go("menu-play", exact=True)
    go("play-song")
    go("play-2d")
    brpctl._go("Traditional")
    brpctl.capture(outdir, "menu-SongList")
    brpctl._go("Amazing Grace")
    brpctl.capture(outdir, "menu-HarpCheck")

    # A lesson reader page.
    brpctl.to_main_menu()
    brpctl.set_state("menu", "LessonTree")
    time.sleep(2.0)
    labels = [label for label, _, _ in brpctl.buttons()]
    lesson = next((l for l in labels if l not in ("←",) and len(l) > 3), None)
    if lesson:
        brpctl._go(lesson)
        brpctl.capture(outdir, "menu-LessonReader")

    # Gameplay, paused, and results — with autoplay so results show hits.
    brpctl.set_autoplay(True)
    brpctl.to_main_menu()
    enter_song("Traditional", "Amazing Grace")
    brpctl.capture(outdir, "play2d")
    brpctl._pause()
    brpctl.capture(outdir, "play2d-paused")
    brpctl.to_main_menu()
    enter_song(*brpctl.SHORTEST_SONG)
    wait_for_results()
    brpctl.capture(outdir, "results")
    brpctl.set_autoplay(False)

    # Jam Session and the Song Editor.
    brpctl.to_main_menu()
    go("menu-play", exact=True)
    go("jam-session")
    go("jam-session-pick-song")
    brpctl._go("Traditional")
    brpctl._go("Amazing Grace")
    go("harp-check-play", exact=True, settle=brpctl.COUNTDOWN_SETTLE)
    brpctl.capture(outdir, "jam")
    brpctl.to_main_menu()
    brpctl.enter_editor("Traditional", "Amazing Grace")
    brpctl.capture(outdir, "editor")
    brpctl.to_main_menu()


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "target/screenshots/locale")
