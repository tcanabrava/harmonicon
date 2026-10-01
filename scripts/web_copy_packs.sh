#!/bin/sh
# SPDX-License-Identifier: MIT
#
# Trunk post_build hook (Trunk.toml): copies the lesson and song packs the
# wasm build embedded into the bundle Trunk is staging, under the asset paths
# the game loads them from — assets/lessons/ and assets/songs/.
#
# wasm can neither download a pack nor list a directory, so the build scripts
# embed the packs' manifests (harmonicon-song's and harmonicon-platform's
# build.rs, from HARMONICON_LESSONS_DIR and HARMONICON_SONGS_DIR) and the
# files themselves must be served beside the game. A variable that is unset
# skips its pack: that build has no lessons or songs, and cargo said so.
#
# Run through scripts/build_web.sh, which sets both.
set -eu

: "${TRUNK_STAGING_DIR:?run by trunk as a post_build hook}"

copy_pack() {
    from=$1
    to="$TRUNK_STAGING_DIR/assets/$2"
    [ -n "$from" ] || return 0
    rm -rf "$to"
    mkdir -p "$to"
    # Content only: the checkout's own .git and CI configuration are not
    # assets, and hidden files are skipped by the game's scans anyway.
    (cd "$from" && find . -path './.*' -prune -o -type f -print) | while read -r file; do
        mkdir -p "$to/$(dirname "$file")"
        cp "$from/$file" "$to/$file"
    done
}

copy_pack "${HARMONICON_LESSONS_DIR:-}" lessons
copy_pack "${HARMONICON_SONGS_DIR:-}" songs
