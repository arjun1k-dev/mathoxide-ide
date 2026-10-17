use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn read_shader(root: &Path, rel_path: &str) -> String {
    let full_path = if rel_path.starts_with("inserts/") {
        root.join(rel_path)
    } else {
        root.join(rel_path)
    };
    let content = fs::read_to_string(&full_path)
        .unwrap_or_else(|e| panic!("Failed to read {}: {}", full_path.display(), e));
    
    let mut result = String::new();
    for line in content.lines() {
        if let Some(rest) = line.strip_prefix("#INSERT ") {
            let insert_name = rest.trim();
            let insert_rel = if insert_name.contains('/') {
                insert_name.to_string()
            } else {
                format!("inserts/{}", insert_name)
            };
            let inserted = read_shader(root, &insert_rel);
            result.push_str(&inserted);
            result.push('\n');
        } else {
            result.push_str(line);
            result.push('\n');
        }
    }
    result
}

fn main() {
    println!("cargo:rerun-if-changed=../../shaders");

    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let root = PathBuf::from("../../shaders");

    let data_layout = r#"
const DATA_STRIDE: u32 = 10u;
const DATA_OFFSET_point: u32 = 0u;
const DATA_OFFSET_stroke_rgba: u32 = 3u;
const DATA_OFFSET_stroke_width: u32 = 7u;
const DATA_OFFSET_subpath_range: u32 = 8u;
"#;

    let mobject_uniforms = r#"
    is_fixed_in_frame: f32,
    shading: vec3f,
    clip_plane0: vec4f,
    clip_plane1: vec4f,
    clip_plane2: vec4f,
    clip_plane3: vec4f,
    anti_alias_width: f32,
    joint_roundness: f32,
    flat_stroke: f32,
    stroke_width_in_scene_units: f32,
    unit_normal: vec3f,
    fill_rgba: vec4f,
    fill_rgba_end: vec4f,
    gradient_start: vec3f,
    gradient_end: vec3f,
    fill_border_width: f32,
"#;

    let raw_fill = read_shader(&root, "fill.wgsl");
    let fill = raw_fill
        .replace("// DATA_LAYOUT", data_layout)
        .replace("// MOBJECT_UNIFORMS", mobject_uniforms);
    fs::write(out_dir.join("fill.wgsl"), fill).expect("write fill.wgsl");

    let raw_stroke = read_shader(&root, "stroke.wgsl");
    let stroke = raw_stroke
        .replace("// DATA_LAYOUT", data_layout)
        .replace("// MOBJECT_UNIFORMS", mobject_uniforms);
    fs::write(out_dir.join("stroke.wgsl"), &stroke).expect("write stroke.wgsl");

    let border = stroke.replace(
        "const IS_FILL_BORDER: bool = false;",
        "const IS_FILL_BORDER: bool = true;",
    );
    fs::write(out_dir.join("border.wgsl"), border).expect("write border.wgsl");

    // Present shader: direct copy from root
    let present_src = fs::read_to_string(root.join("present.wgsl"))
        .expect("read present.wgsl");
    fs::write(out_dir.join("present.wgsl"), &present_src).expect("write present.wgsl");

    // Present shader for sRGB target: converts sRGB texture to linear so sRGB surface doesn't double-gamma
    let present_srgb_src = r#"
@group(0) @binding(0) var frame_texture: texture_2d<f32>;
@group(0) @binding(1) var frame_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) im_coords: vec2f,
}

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOutput {
    var corners = array<vec2f, 3>(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0));
    let corner = corners[index];

    var out: VertexOutput;
    out.position = vec4f(corner, 0.0, 1.0);
    out.im_coords = vec2f(0.5 + 0.5 * corner.x, 0.5 - 0.5 * corner.y);
    return out;
}

fn srgb_to_linear(c: vec4f) -> vec4f {
    let rgb = select(
        c.rgb / 12.92,
        pow((c.rgb + vec3f(0.055)) / 1.055, vec3f(2.4)),
        c.rgb > vec3f(0.04045)
    );
    return vec4f(rgb, c.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    let c = textureSample(frame_texture, frame_sampler, in.im_coords);
    return srgb_to_linear(c);
}
"#;
    fs::write(out_dir.join("present_srgb.wgsl"), present_srgb_src).expect("write present_srgb.wgsl");
}
