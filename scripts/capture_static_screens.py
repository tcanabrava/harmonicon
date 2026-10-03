#!/usr/bin/env python3
"""Capture the screens that hold still, for a pixel diff across a refactor.

`brpctl.py --take-all-screenshots` covers gameplay, where the highway moves
and two runs never match pixel for pixel. These screens don't move, so a
before/after pair should be identical unless the change altered them:

    python3 scripts/capture_static_screens.py target/shots/before
    # ...change, rebuild, relaunch...
    python3 scripts/capture_static_screens.py target/shots/after
    python3 scripts/capture_static_screens.py --diff target/shots/before target/shots/after

Needs a running `--features dev` build. The editor chart comes from the
`~/Harmonicon/songs` drop folder, which is where the editor's Load dialog
opens; pass `--artist`/`--song` to pick a different one.
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.dirname(__file__))
import brpctl  # noqa: E402


def capture_all(outdir, artist, song):
    brpctl.resize(1920, 1080)
    time.sleep(1.0)
    brpctl.to_main_menu()
    brpctl.capture(outdir, "menu-home")
    brpctl.set_state("menu", "Options")
    time.sleep(3.0)
    brpctl.capture(outdir, "menu-options")
    # Credits scrolls, so only its first frames are comparable: capture as
    # soon as it settles rather than after a fixed wait.
    brpctl.set_state("app", "Credits")
    time.sleep(1.5)
    brpctl.capture(outdir, "credits")
    # The editor's Details tab is a `RadioButton`, which reacts to a real
    # pointer click only — `brpctl.click`'s remote `Activate` can't open it.
    brpctl.enter_editor(artist, song)
    brpctl.capture(outdir, "editor-chart")


def diff(before, after):
    from PIL import Image, ImageChops

    changed = 0
    for name in sorted(os.listdir(before)):
        other = os.path.join(after, name)
        if not name.endswith(".png") or not os.path.exists(other):
            continue
        a = Image.open(os.path.join(before, name)).convert("RGB")
        b = Image.open(other).convert("RGB")
        box = ImageChops.difference(a, b).getbbox() if a.size == b.size else "size"
        changed += box is not None
        print(f"{name}: {'identical' if box is None else f'differs in {box}'}")
    return changed


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("outdir", nargs="?")
    parser.add_argument("--diff", nargs=2, metavar=("BEFORE", "AFTER"))
    parser.add_argument("--artist", default="Chicago Nights")
    parser.add_argument("--song", default="Windy City Swing")
    args = parser.parse_args()
    if args.diff:
        sys.exit(1 if diff(*args.diff) else 0)
    if not args.outdir:
        parser.error("an output directory, or --diff BEFORE AFTER")
    capture_all(args.outdir, args.artist, args.song)


if __name__ == "__main__":
    main()
