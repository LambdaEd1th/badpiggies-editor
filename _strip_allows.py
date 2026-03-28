"""Strip all the #[allow(...)] annotations we added so clippy shows raw warnings."""
import re, pathlib, subprocess

files = [
    "src/app.rs",
    "src/assets.rs",
    "src/parser.rs",
    "src/renderer/bg_shader.rs",
    "src/renderer/background.rs",
    "src/renderer/compounds.rs",
    "src/renderer/edge_shader.rs",
    "src/renderer/mod.rs",
    "src/renderer/opaque_shader.rs",
    "src/renderer/sprite_shader.rs",
    "src/renderer/sprites.rs",
    "src/renderer/terrain.rs",
    "src/types.rs",
]

# Each pattern: (old, new) — plain string substitutions for the allows we added.
# Lines like `#[allow(dead_code)]\n` directly before pub/fn/const/field lines.
ALLOWS = [
    "#[allow(dead_code)]\npub const LAYER_WORLD_OBJECT",
    "#[allow(dead_code)]\npub fn is_compound",
    "#[allow(dead_code)]\nconst BRIDGE_ROPE",
    "#[allow(dead_code, clippy::too_many_arguments)]\nfn draw_sub_sprites(",
    "#[allow(dead_code, clippy::too_many_arguments)]\npub fn make_edge_paint_callback",
    "#[allow(dead_code)]\nstruct OpaquePaintCallback",
    "#[allow(dead_code, clippy::too_many_arguments)]\npub fn make_opaque_sprite_callback",
    "#[allow(dead_code)]\nstruct SpritePaintCallback",
    "#[allow(dead_code)]\npub fn make_sprite_callback",
    "#[allow(dead_code, clippy::too_many_arguments)]\npub fn draw_sprite",
    "#[allow(dead_code)]\npub fn transform_mesh_to_screen",
    "#[allow(dead_code)]\npub fn load_from_rgba",
    "#[allow(dead_code)]\npub fn clear",
    "#[allow(dead_code)]\n    pub fn position",
    "#[allow(dead_code)]\n    pub fn remaining",
    "#[allow(dead_code)]\n    pub fn read_uint16",
    "#[allow(dead_code)]\n    len: usize,",
    "#[allow(dead_code)]\nfn show_properties",
    "#[allow(dead_code)]\nfn show_vec3",
    "#[allow(dead_code)]\n    z: f32,",
    "#[allow(dead_code)]\n    pub is_dessert: bool,",
    "#[allow(clippy::large_enum_variant)]\npub enum LevelObject",
    "#[allow(clippy::too_many_arguments)]\npub fn draw_bg_layers",
    "#[allow(clippy::too_many_arguments)]\nfn draw_bg_sprite_offset",
    "#[allow(clippy::too_many_arguments)]\npub fn draw_compound",
    "#[allow(clippy::too_many_arguments)]\npub fn draw_bird_face",
    "#[allow(clippy::too_many_arguments)]\nfn draw_sub_sprites_rotated",
    "#[allow(clippy::too_many_arguments)]\nfn draw_uv_quad(",
    "#[allow(clippy::too_many_arguments)]\nfn draw_uv_quad_rotated",
    "#[allow(clippy::too_many_arguments)]\npub fn upload_edge_mesh",
    "#[allow(clippy::too_many_arguments)]\npub fn make_single_edge_paint_callback",
    "#[allow(dead_code, clippy::too_many_arguments)]\npub fn make_edge_paint_callback",
    "#[allow(clippy::too_many_arguments)]\npub fn build_quad",
    "/// Draw Bird face sprite AFTER the body has been rendered, so it appears in front.\n/// `world_y` should already include the sleep bob offset.\n/// `breath_sx`/`breath_sy` are the hermite-evaluated scale factors from the vizGroup.\n#[allow(clippy::too_many_arguments)]\npub fn draw_bird_face",
]

for fpath in files:
    p = pathlib.Path(fpath)
    if not p.exists():
        continue
    src = p.read_text()
    orig = src
    for allow_line in ALLOWS:
        # allow_line = "#[allow(...)]\nFN_SIG" → strip the allow+newline
        tag = allow_line.split("\n")[0]  # just the #[allow(...)] part
        rest = "\n".join(allow_line.split("\n")[1:])  # what follows
        src = src.replace(allow_line, rest)
    if src != orig:
        p.write_text(src)
        print(f"Stripped: {fpath}")
    else:
        print(f"No change: {fpath}")

print("Done. Run: cargo clippy to check remaining warnings")
