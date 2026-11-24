// WGSL shading pass for the GPU viewport (wave D, card UI-D2).
//
// Unlike the opaque `viewport.wgsl` passthrough, this shader owns per-fragment
// studio lighting: the host hands it NDC position (pre-projected by the CPU,
// per the D1 projection model), a VIEW-SPACE normal and a BASE colour + alpha,
// and the fragment stage computes `key + fill` diffuse lighting so flat vs
// smooth normals and the transparency alpha reach the pixels.
//
// The light rig values are injected at build time from `crate::gpu::shading`
// (`shading_wgsl()`), so the WGSL and the Rust constants are one source of
// truth. The placeholders are the only thing substituted; nothing else in this
// file may change.

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    // `pos` carries already-divided NDC (x/y in [-1,1], z unused); w=1 so the
    // rasterizer's own divide is a no-op.
    out.clip = vec4<f32>(in.pos.x, in.pos.y, 0.0, 1.0);
    out.normal = in.normal;
    out.color = in.color;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    // Studio light rig (view-space, injected from shading.rs).
    let key_dir = normalize(vec3<f32>(__KEY_DIR_X__, __KEY_DIR_Y__, __KEY_DIR_Z__));
    let fill_dir = normalize(vec3<f32>(__FILL_DIR_X__, __FILL_DIR_Y__, __FILL_DIR_Z__));
    let key_strength = __KEY_STRENGTH__;
    let fill_strength = __FILL_STRENGTH__;
    let ambient = __AMBIENT__;

    let n = normalize(in.normal);
    let diffuse = ambient
        + key_strength * max(dot(n, key_dir), 0.0)
        + fill_strength * max(dot(n, fill_dir), 0.0);
    let light = clamp(diffuse, 0.0, 1.0);
    return vec4<f32>(in.color.rgb * light, in.color.a);
}
