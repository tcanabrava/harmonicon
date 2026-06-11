import numpy as np
import trimesh

def create_low_poly_harp():
    # ---------------------------------------------------------
    # 1. Dimensions (in millimeters)
    # ---------------------------------------------------------
    length = 100.0
    width = 27.0
    comb_thickness = 6.0
    cover_thickness = 2.0

    # ---------------------------------------------------------
    # 2. Materials (PBR configuration for glTF)
    # ---------------------------------------------------------
    # Mouthpiece / Comb: Matte Brown Plastic/Wood
    mouthpiece_material = trimesh.visual.material.PBRMaterial(
        baseColorFactor=[101/255, 67/255, 33/255, 1.0],  # #654321 Dark Brown
        roughnessFactor=0.6,
        metallicFactor=0.0
    )

    # Upper & Lower Caps: Polished Silver Metal
    cap_material = trimesh.visual.material.PBRMaterial(
        baseColorFactor=[0.85, 0.85, 0.85, 1.0],         # Silver Plating
        roughnessFactor=0.2,
        metallicFactor=0.9
    )

    # ---------------------------------------------------------
    # 3. Create the Mouthpiece (Comb) with 10 Precise Holes
    # ---------------------------------------------------------
    # Main comb block
    comb_base = trimesh.creation.box(extents=[length, width, comb_thickness])

    # Hole configuration
    hole_w = 4.5
    hole_h = comb_thickness - 1.5
    hole_depth = 12.0  # Depth from the front face
    spacing = 2.5

    # Center-aligned X start coordinate for the 10 holes
    total_holes_width = (10 * hole_w) + (9 * spacing)
    start_x = -(total_holes_width / 2) + (hole_w / 2)

    # Generate the 10 hole cutout meshes
    hole_cutouts = []
    for i in range(10):
        x_pos = start_x + i * (hole_w + spacing)
        # Position cutouts right at the front edge
        y_pos = (width / 2) - (hole_depth / 2) + 0.1

        hole_box = trimesh.creation.box(extents=[hole_w, hole_depth, hole_h])
        hole_box.apply_translation([x_pos, y_pos, 0])
        hole_cutouts.append(hole_box)

    # Combine cutouts and execute a clean geometric boolean subtraction
    all_holes = trimesh.util.concatenate(hole_cutouts)
    mouthpiece = comb_base.difference(all_holes)

    # Re-apply the brown material explicitly to the processed mesh
    mouthpiece.visual.material = mouthpiece_material

    # ---------------------------------------------------------
    # 4. Create Upper and Lower Metallic Caps
    # ---------------------------------------------------------
    # Upper Cap (shifted up along Z-axis)
    upper_cap = trimesh.creation.box(extents=[length + 0.2, width + 0.2, cover_thickness])
    z_offset_upper = (comb_thickness / 2) + (cover_thickness / 2)
    upper_cap.apply_translation([0, 0, z_offset_upper])
    upper_cap.visual.material = cap_material

    # Lower Cap (shifted down along Z-axis)
    lower_cap = trimesh.creation.box(extents=[length + 0.2, width + 0.2, cover_thickness])
    z_offset_lower = -((comb_thickness / 2) + (cover_thickness / 2))
    lower_cap.apply_translation([0, 0, z_offset_lower])
    lower_cap.visual.material = cap_material

    # ---------------------------------------------------------
    # 5. Assemble into a Scene and Export
    # ---------------------------------------------------------
    scene = trimesh.Scene([mouthpiece, upper_cap, lower_cap])

    output_filename = "low_poly_blues_harmonica.glb"
    scene.export(output_filename)
    print(f"Success! Exported clean low-poly model to '{output_filename}'")

if __name__ == "__main__":
    create_low_poly_harp()
