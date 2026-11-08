#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Regenerate CODE_MAP.md: one line per Rust source file, grouped by crate.

Each line is the file's path, its size in lines, and the first sentence of
its `//!` header, so "where does X live" is a grep of one file instead of a
search of the tree. Everything in CODE_MAP.md above the GENERATED marker is
hand-written and kept as it is; everything below is replaced.

    python3 scripts/code_map.py          # rewrite CODE_MAP.md
    python3 scripts/code_map.py --check  # exit 1 if it is out of date

Standard library only. Test files (`tests.rs`, `tests/`) are left out: they
sit beside the module they test, which the map already names.
"""

import re
import subprocess
import sys
from collections import defaultdict
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAP = ROOT / "CODE_MAP.md"
MARKER = "<!-- GENERATED BELOW by scripts/code_map.py; edit above this line only -->"
SUMMARY_CHARS = 150


def tracked_sources():
    out = subprocess.run(
        ["git", "ls-files", "*.rs"], cwd=ROOT, capture_output=True, text=True, check=True
    ).stdout.split()
    return sorted(
        p for p in out if not (p.endswith("/tests.rs") or "/tests/" in p or p.startswith("tests/"))
    )


def summary(text):
    """The first sentence of the leading `//!` block, plain text."""
    lines = []
    for line in text.splitlines():
        if line.startswith("//!"):
            body = line[3:].strip()
            if not body and lines:
                break
            if body:
                lines.append(body)
        elif lines:
            break
    if not lines:
        return ""
    joined = re.sub(r"\[`?([^\]`]+)`?\]\([^)]*\)", r"\1", " ".join(lines))  # [x](y) -> x
    joined = re.sub(r"\[`([^`]+)`\]", r"`\1`", joined)  # [`x`] -> `x`
    sentence = re.split(r"(?<=[.!?])\s", joined, maxsplit=1)[0]
    if len(sentence) > SUMMARY_CHARS:
        sentence = sentence[: SUMMARY_CHARS - 1].rstrip() + "…"
    return sentence


def group(path):
    parts = path.split("/")
    return parts[1] if parts[0] == "crates" else "harmonicon (root)"


def generated():
    groups = defaultdict(list)
    for path in tracked_sources():
        text = (ROOT / path).read_text(encoding="utf-8", errors="replace")
        lines = text.count("\n")
        about = summary(text) or "(no `//!` header)"
        groups[group(path)].append(f"- `{path}` ({lines}) — {about}")
    out = []
    for name in sorted(groups):
        out.append(f"\n### {name}\n")
        out.extend(groups[name])
    return "\n".join(out).lstrip("\n") + "\n"


def main():
    existing = MAP.read_text(encoding="utf-8") if MAP.exists() else ""
    head = existing.split(MARKER)[0].rstrip() if MARKER in existing else "# Code map"
    content = f"{head}\n\n{MARKER}\n\n{generated()}"
    if "--check" in sys.argv:
        if existing != content:
            print("CODE_MAP.md is out of date: run python3 scripts/code_map.py")
            sys.exit(1)
        return
    MAP.write_text(content, encoding="utf-8")
    print(f"wrote {MAP.relative_to(ROOT)}")


if __name__ == "__main__":
    main()
