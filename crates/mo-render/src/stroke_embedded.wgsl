// MathOxide stroke shader: flat ribbon quads generated on CPU per curve.

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
};

@vertex
fn vs_main(@location(0) pos: vec3f, @location(1) color: vec4f) -> VertexOutput {
    var out: VertexOutput;
    let c = frame.camera_position;
    out.position = vec4f(
        (pos.x - c.x) * frame.frame_rescale_factors.x,
        (pos.y - c.y) * frame.frame_rescale_factors.y,
        clamp(pos.z * 0.1, -1.0, 1.0),
        1.0
    );
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4f {
    if (in.color.a == 0.0) { discard; }
    return in.color;
}
