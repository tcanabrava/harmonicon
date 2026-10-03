#!/usr/bin/env python3
"""Find repeated runs of Rust source lines across the workspace.

Lines are normalised (whitespace collapsed, comments and blank lines and
lone braces dropped) and every window of `--window` consecutive normalised
lines is hashed. Windows that occur more than once are merged into maximal
runs and reported largest first, so a 40-line copy shows up once as a
40-line clone rather than as 33 overlapping 8-line ones.

    python3 scripts/find_duplicates.py                 # non-test code
    python3 scripts/find_duplicates.py --tests         # include test code
    python3 scripts/find_duplicates.py --window 6 --top 80
"""

import argparse
import collections
import re
import subprocess

TRIVIAL = {"{", "}", "};", "})", "});", ")", ");", "),", "},", "]", "],", "..default()", "..Default::default()"}


def normalised_lines(path, include_tests):
    out = []
    whole_file_test = path.startswith("tests/") or "/tests/" in path or path.endswith("tests.rs")
    if whole_file_test and not include_tests:
        return out
    with open(path, encoding="utf-8") as f:
        for lineno, raw in enumerate(f, 1):
            line = raw.strip()
            if not include_tests and line.startswith("#[cfg(test)]"):
                break
            if not line or line.startswith("//") or line in TRIVIAL:
                continue
            if line.startswith("use ") or line.startswith("#[derive"):
                continue
            out.append((lineno, re.sub(r"\s+", " ", line)))
    return out


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--window", type=int, default=8)
    parser.add_argument("--top", type=int, default=60)
    parser.add_argument("--tests", action="store_true")
    args = parser.parse_args()

    files = subprocess.run(
        ["git", "ls-files", "*.rs"], capture_output=True, text=True, check=True
    ).stdout.split()

    lines = {p: normalised_lines(p, args.tests) for p in files}
    windows = collections.defaultdict(list)
    for path, ls in lines.items():
        for i in range(len(ls) - args.window + 1):
            key = "\n".join(t for _, t in ls[i : i + args.window])
            windows[key].append((path, i))

    dup_at = collections.defaultdict(set)
    for occ in windows.values():
        if len(occ) > 1:
            for path, i in occ:
                dup_at[path].update(range(i, i + args.window))

    # Merge each file's duplicated normalised lines into maximal runs.
    runs = []
    for path, idx in dup_at.items():
        idx = sorted(idx)
        start = prev = idx[0]
        for i in idx[1:] + [None]:
            if i is not None and i == prev + 1:
                prev = i
                continue
            ls = lines[path]
            runs.append((prev - start + 1, path, ls[start][0], ls[prev][0], ls[start][1]))
            if i is not None:
                start = prev = i

    total = sum(r[0] for r in runs)
    print(f"{total} normalised lines sit inside a repeated {args.window}-line window\n")
    by_file = collections.Counter()
    for n, path, *_ in runs:
        by_file[path] += n
    print("by file:")
    for path, n in by_file.most_common(args.top // 2):
        print(f"{n:>6}  {path}")
    print("\nlargest runs:")
    for n, path, a, b, first in sorted(runs, reverse=True)[: args.top]:
        print(f"{n:>5}  {path}:{a}-{b}   {first[:70]}")


if __name__ == "__main__":
    main()
