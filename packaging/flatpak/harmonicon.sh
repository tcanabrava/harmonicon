#!/bin/sh
# Flatpak launcher for Harmonicon.
#
# Bevy resolves its `assets/` folder relative to BEVY_ASSET_ROOT, and a few code
# paths read asset files relative to the working directory, so point both at the
# bundled data root before launching the real binary.
export BEVY_ASSET_ROOT="/app/share/harmonicon"
cd "/app/share/harmonicon" || exit 1
exec /app/bin/harmonicon.bin "$@"
