#!/bin/sh
# SPDX-License-Identifier: MIT
#
# Builds (or serves) the web version with its lessons and songs.
#
#   scripts/build_web.sh                   # trunk build --release
#   scripts/build_web.sh serve             # trunk serve --release
#   scripts/build_web.sh build             # any trunk subcommand and flags
#
# The packs are fetched from their official repositories — only the latest
# commit — into target/web-packs/. To build with other checkouts instead
# (yours, while authoring), point HARMONICON_LESSONS_DIR and
# HARMONICON_SONGS_DIR at them; those are used as they are.
#
# Both variables are read twice: by the build scripts, which embed the packs'
# manifests in the wasm (a browser can't list a directory), and by Trunk's
# post_build hook (scripts/web_copy_packs.sh), which copies the files into the
# bundle under assets/lessons and assets/songs.
set -eu

cd "$(dirname "$0")/.."
packs="$PWD/target/web-packs"

latest() {
    name=$1
    dir="$packs/$name"
    url="https://github.com/tcanabrava/harmonicon-$name"
    if [ -d "$dir/.git" ]; then
        git -C "$dir" fetch --quiet --depth 1 origin HEAD
        git -C "$dir" reset --quiet --hard FETCH_HEAD
    else
        mkdir -p "$packs"
        git clone --quiet --depth 1 "$url" "$dir"
    fi
    echo "$dir"
}

absolute() {
    (cd "$1" && pwd)
}

if [ -n "${HARMONICON_LESSONS_DIR:-}" ]; then
    HARMONICON_LESSONS_DIR=$(absolute "$HARMONICON_LESSONS_DIR")
else
    HARMONICON_LESSONS_DIR=$(latest lessons)
fi
if [ -n "${HARMONICON_SONGS_DIR:-}" ]; then
    HARMONICON_SONGS_DIR=$(absolute "$HARMONICON_SONGS_DIR")
else
    HARMONICON_SONGS_DIR=$(latest songs)
fi
export HARMONICON_LESSONS_DIR HARMONICON_SONGS_DIR

echo "lessons: $HARMONICON_LESSONS_DIR"
echo "songs:   $HARMONICON_SONGS_DIR"

# getrandom's wasm backend needs this cfg, which Trunk can't set (Trunk.toml).
export RUSTFLAGS="${RUSTFLAGS:-} --cfg getrandom_backend=\"wasm_js\""

if [ $# -eq 0 ]; then
    set -- build --release
fi
exec trunk "$@"
