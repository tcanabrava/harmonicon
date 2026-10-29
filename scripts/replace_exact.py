#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Apply exact, count-checked string replacements across files.

Usage: replace_exact.py EDITS.json

EDITS.json maps each file path to a list of [old, new] pairs, applied in
order. Each `old` must occur exactly once in its file as it stands when
that pair is applied. Every file is checked before any is written, so a
refactor either applies completely or not at all, and a failure names the
file and pair that didn't match.
"""

import json
import sys


def main() -> int:
    if len(sys.argv) != 2:
        print(__doc__, file=sys.stderr)
        return 2
    with open(sys.argv[1], encoding="utf-8") as f:
        plan = json.load(f)

    results = {}
    for path, edits in plan.items():
        with open(path, encoding="utf-8") as f:
            text = f.read()
        for i, (old, new) in enumerate(edits):
            count = text.count(old)
            if count != 1:
                print(f"{path}: edit {i} matches {count} times:\n{old}", file=sys.stderr)
                return 1
            text = text.replace(old, new)
        results[path] = text

    for path, text in results.items():
        with open(path, "w", encoding="utf-8") as f:
            f.write(text)
    return 0


if __name__ == "__main__":
    sys.exit(main())
