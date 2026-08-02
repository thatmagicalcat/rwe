struct Uniforms {
    screen_size: vec2<f32>,
    time: f32,
    anim_time: f32,
    delay_spread: f32,
    max_distance: f32,
    cursor: vec2<f32>,
    radius: f32,
    mode: u32,
    _pad: vec2<f32>,
    colors: array<vec4<f32>, 4>,
};

struct GpuTriangle {
    pa: vec2<f32>,
    pb: vec2<f32>,
    pc: vec2<f32>,
    a_start: vec2<f32>,
    distance: f32,
    palette_index: u32,
};

struct VertexInput {
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
};

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;
@group(0) @binding(1) var<storage, read> triangles: array<GpuTriangle>;

@vertex
fn vs_main(in: VertexInput) -> VertexOutput {
    let triangle = triangles[in.instance_index];
    let delay = u.delay_spread * (triangle.distance / u.max_distance);
    let base = (triangle.pb + triangle.pc) * 0.5;
    let distance = length(base - u.cursor);
    let t = clamp(1.0 - (distance - u.radius * 0.3) / (u.radius * 1.2), 0.0, 1.0);
    let et = cubic_out(t);
    let animated_a = mix(triangle.a_start, triangle.pa, et);

    var pos: vec2<f32>;
    switch in.vertex_index {
        case 0: { pos = animated_a; }
        case 1: { pos = triangle.pb; }
        default: { pos = triangle.pc; }
    }

    let ndc = pos / u.screen_size * 2.0 - 1.0;

    let alpha_t = quad_out(t);
    var out: VertexOutput;

    out.position = vec4<f32>(ndc * vec2<f32>(1.0, -1.0), 0.0, 1.0);
    out.color = u.colors[triangle.palette_index % 4u];
    out.color.a *= alpha_t;
    // let final_color = vec4<f32>(base_color.rgb * alpha_t, base_color.a * alpha_t * (200.0 / 255.0));

    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
}

fn cubic_out(t: f32) -> f32 {
    let f = t - 1.0;
    return f * f * f + 1.0;
}

fn quad_out(t: f32) -> f32 {
    return -t * (t - 2.0);
}
