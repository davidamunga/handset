struct U {
    rect: vec4<f32>, // x, y, w, h in window space (0..1, origin top-left)
    extra: vec4<f32>, // x: 1 = sample the texture rotated 90 degrees clockwise; y: opacity
    // Rows of a homography on window space (0..1); the identity for layers that stay flat.
    h0: vec4<f32>,
    h1: vec4<f32>,
    h2: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: U;
@group(0) @binding(1) var samp: sampler;
@group(0) @binding(2) var tex_a: texture_2d<f32>;
@group(0) @binding(3) var tex_b: texture_2d<f32>;
@group(0) @binding(4) var tex_c: texture_2d<f32>;

struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs(@builtin(vertex_index) i: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2(0.0, 0.0), vec2(1.0, 0.0), vec2(0.0, 1.0),
        vec2(0.0, 1.0), vec2(1.0, 0.0), vec2(1.0, 1.0),
    );
    let c = corners[i];
    let p = u.rect.xy + c * u.rect.zw;
    var o: VOut;
    let q = vec3(p, 1.0);
    let x = dot(u.h0.xyz, q);
    let y = dot(u.h1.xyz, q);
    let w = dot(u.h2.xyz, q);
    // homogeneous output keeps the texture mapping perspective-correct
    o.pos = vec4(x * 2.0 - w, w - y * 2.0, 0.0, w);
    // Source uv of a 90 degree clockwise rotation: dest (x, y) <- src (y, 1 - x)
    o.uv = select(c, vec2(c.y, 1.0 - c.x), u.extra.x > 0.5);
    return o;
}

// Android's encoder output is BT.709, limited range.
@fragment
fn fs_video(in: VOut) -> @location(0) vec4<f32> {
    let y = (textureSample(tex_a, samp, in.uv).r - 16.0 / 255.0) * (255.0 / 219.0);
    let cb = textureSample(tex_b, samp, in.uv).r - 128.0 / 255.0;
    let cr = textureSample(tex_c, samp, in.uv).r - 128.0 / 255.0;
    let uu = cb * (255.0 / 224.0);
    let vv = cr * (255.0 / 224.0);
    let rgb = vec3(
        y + 1.5748 * vv,
        y - 0.1873 * uu - 0.4681 * vv,
        y + 1.8556 * uu,
    );
    return vec4(clamp(rgb, vec3(0.0), vec3(1.0)), 1.0);
}

@fragment
fn fs_solid(in: VOut) -> @location(0) vec4<f32> {
    return vec4(0.0, 0.0, 0.0, 1.0);
}

// The bezel PNG has straight alpha; the window surface is premultiplied.
@fragment
fn fs_bezel(in: VOut) -> @location(0) vec4<f32> {
    let c = textureSample(tex_a, samp, in.uv);
    let a = c.a * u.extra.y;
    return vec4(c.rgb * a, a);
}
