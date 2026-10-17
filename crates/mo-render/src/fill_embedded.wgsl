// MathOxide fill shader — technique ported from ManimGL's fill.wgsl:
// fan triangle covers interior, curve triangle is cut in uv space where the
// bezier coincides with y = x^2. v0.1 renders winding cut without stencil
// counting (single draw); interior correctness comes from fan + cut curves.

struct FrameUniforms {
    view: mat4x4f,
    frame_rescale_factors: vec3f,
    frame_scale: f32,
    camera_position: vec3f,
    pixel_size: f32,
    light_position: vec3f,
};
@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VertexOutput {
    @builtin(position) position: vec4f,
    @location(0) color: vec4f,
    @location(1) uv_coords: vec2f,
    @location(2) fill_all: f32,
};

@vertex
fn vs_main(@location(0) pos: vec3f, @location(1) color: vec4f,
           @location(2) uv: vec2f, @location(3) fill_all: f32) -> VertexOutput {
    var out: VertexOutput;
    // Orthographic: scene units -> NDC via frame rescale factors.
    let c = frame.camera_position;
    out.position = vec4f(
        (pos.x - c.x) * frame.frame_rescale_factors.x,
        (pos.y - c.y) * frame.frame_rescale_factors.y,
        clamp(pos.z * 0.1, -1.0, 1.0),
        1.0
    );
    out.color = color;
    out.uv_coords = uv;
    out.fill_all = fill_all;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    if (in.color.a == 0.0) { discard; }
    if (in.fill_all == 0.0 && in.uv_coords.y < in.uv_coords.x * in.uv_coords.x) { discard; }
    return in.color;
}
