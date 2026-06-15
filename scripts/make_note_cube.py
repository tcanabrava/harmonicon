# SPDX-License-Identifier: MIT
"""Generate the 3D comet-head mesh: a cube with elongated sides, centered at the
origin so the game can scale/position it. Exported as glb for both note themes."""
import trimesh

# Elongated along Z (the lane/travel axis) so the head reads as a little ingot.
head = trimesh.creation.box(extents=[1.0, 0.8, 1.4])
scene = trimesh.Scene(head)
for name in ("circular", "square"):
    out = f"assets/notes/3d/{name}.glb"
    scene.export(out)
    print("wrote", out)
