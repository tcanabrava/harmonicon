#!/usr/bin/env python3
"""Break the workspace's Rust line count down by crate and by kind.

Each tracked `.rs` file's lines are classified as blank, comment (`//`,
`///`, `//!`, or inside a `/* */` block), test (anything from a
`#[cfg(test)]` item onward, or a whole file under `tests/` or named
`tests.rs`), or code. The test split is a heuristic: it assumes a file's
`#[cfg(test)] mod tests` comes last, which is this repo's convention.

    python3 scripts/loc_report.py            # per-crate table
    python3 scripts/loc_report.py --files 40 # plus the 40 largest code files
"""

import argparse
import collections
import subprocess


def classify(path):
    counts = collections.Counter()
    whole_file_test = (
        path.startswith("tests/") or "/tests/" in path or path.endswith("tests.rs")
    )
    in_test = whole_file_test
    in_block = False
    with open(path, encoding="utf-8") as f:
        for raw in f:
            line = raw.strip()
            if not in_test and line.startswith("#[cfg(test)]"):
                in_test = True
            if not line:
                counts["blank"] += 1
            elif in_block or line.startswith("/*"):
                counts["comment"] += 1
                in_block = "*/" not in line
            elif line.startswith("//"):
                counts["comment"] += 1
            elif in_test:
                counts["test"] += 1
            else:
                counts["code"] += 1
    return counts


def crate_of(path):
    parts = path.split("/")
    if parts[0] == "crates":
        return parts[1]
    return "(root) " + parts[0]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--files", type=int, default=0)
    args = parser.parse_args()

    files = subprocess.run(
        ["git", "ls-files", "*.rs"], capture_output=True, text=True, check=True
    ).stdout.split()

    per_crate = collections.defaultdict(collections.Counter)
    per_file = {}
    for path in files:
        c = classify(path)
        per_file[path] = c
        per_crate[crate_of(path)].update(c)

    kinds = ["code", "test", "comment", "blank"]
    print(f"{'crate':<28}" + "".join(f"{k:>9}" for k in kinds) + f"{'total':>9}")
    total = collections.Counter()
    for crate, c in sorted(per_crate.items(), key=lambda kv: -kv[1]["code"]):
        total.update(c)
        print(f"{crate:<28}" + "".join(f"{c[k]:>9}" for k in kinds) + f"{sum(c.values()):>9}")
    print(f"{'TOTAL':<28}" + "".join(f"{total[k]:>9}" for k in kinds) + f"{sum(total.values()):>9}")

    if args.files:
        print()
        print(f"{'code':>6} {'comment':>8}  file")
        ranked = sorted(per_file.items(), key=lambda kv: -kv[1]["code"])
        for path, c in ranked[: args.files]:
            print(f"{c['code']:>6} {c['comment']:>8}  {path}")


if __name__ == "__main__":
    main()
