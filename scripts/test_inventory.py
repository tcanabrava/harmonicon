#!/usr/bin/env python3
# SPDX-License-Identifier: MIT
"""Inventory the workspace's #[test] functions, to find ones worth pruning.

Usage:
    python3 scripts/test_inventory.py summary          # tests per file, largest first
    python3 scripts/test_inventory.py tiny [N]         # bodies of at most N lines (default 3)
    python3 scripts/test_inventory.py duplicates       # test bodies identical to another's
    python3 scripts/test_inventory.py grep PATTERN     # tests whose name matches PATTERN

A text scan, not a compiler: it finds `#[test]` followed by `fn name`, and
takes the body as the brace-balanced block after it. Good enough to point a
human at candidates; every removal still needs reading the test.
"""

import re
import subprocess
import sys
from collections import Counter, defaultdict


def rust_files():
    out = subprocess.run(
        ["git", "ls-files", "*.rs"], capture_output=True, text=True, check=True
    ).stdout.split()
    return [p for p in out if not p.startswith("target/")]


TEST_RE = re.compile(r"#\[test\]\s*(?:#\[[^\]]*\]\s*)*fn\s+(\w+)\s*\(")


def tests_in(path):
    text = open(path, encoding="utf-8").read()
    for m in TEST_RE.finditer(text):
        start = text.index("{", m.end())
        depth, i = 0, start
        while i < len(text):
            if text[i] == "{":
                depth += 1
            elif text[i] == "}":
                depth -= 1
                if depth == 0:
                    break
            i += 1
        body = text[start + 1 : i]
        line = text.count("\n", 0, m.start()) + 1
        lines = [l.strip() for l in body.splitlines() if l.strip() and not l.strip().startswith("//")]
        yield m.group(1), line, lines


def inventory():
    for path in rust_files():
        for name, line, body in tests_in(path):
            yield path, name, line, body


def main():
    cmd = sys.argv[1] if len(sys.argv) > 1 else "summary"
    tests = list(inventory())
    if cmd == "summary":
        per_file = Counter(p for p, *_ in tests)
        print(f"{len(tests)} tests in {len(per_file)} files")
        for path, n in per_file.most_common(40):
            print(f"{n:5}  {path}")
    elif cmd == "tiny":
        limit = int(sys.argv[2]) if len(sys.argv) > 2 else 3
        for path, name, line, body in tests:
            if len(body) <= limit:
                print(f"{path}:{line}  {name}  ({len(body)})  {' | '.join(body)[:160]}")
    elif cmd == "duplicates":
        by_body = defaultdict(list)
        for path, name, line, body in tests:
            by_body["\n".join(body)].append(f"{path}:{line} {name}")
        for body, where in by_body.items():
            if len(where) > 1 and body:
                print("---")
                for w in where:
                    print("  " + w)
    elif cmd == "grep":
        pattern = re.compile(sys.argv[2])
        for path, name, line, body in tests:
            if pattern.search(name):
                print(f"{path}:{line}  {name}  ({len(body)})")
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
