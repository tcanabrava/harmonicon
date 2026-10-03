#!/bin/sh
# Symlinks this repo's hooks into .git/hooks. Git does not version-control
# hooks, so each clone runs this once.
set -e
root=$(git rev-parse --show-toplevel)
for hook in "$root"/scripts/git-hooks/*; do
    name=$(basename "$hook")
    case "$name" in install.sh) continue ;; esac
    ln -sf "../../scripts/git-hooks/$name" "$root/.git/hooks/$name"
    echo "installed $name"
done
git -C "$root" config blame.ignoreRevsFile .git-blame-ignore-revs
echo "blame skips the commits in .git-blame-ignore-revs"
