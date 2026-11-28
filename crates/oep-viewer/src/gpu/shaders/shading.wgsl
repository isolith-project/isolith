// WGSL shading pass for the GPU viewport (wave D, cards UI-D2 + UI-D8).
//
// The host hands it a WORLD-space position (the vertex buffer carries world
// coordinates now, NOT pre-projected NDC: UI-D8 moved the transform into the
// vertex stage), a VIEW-SPACE normal and a BASE colour + alpha, and the vertex
// stage applies the MVP identity from the uniform buffer:
//
//     clip = u_mvp · vec4(world, 1)      # world -> homogeneous clip
//     out.clip.xyz = clip.xyz / clip.w   # perspective divide, owned by the
//                                        #   vertex stage
//     out.clip.w   = 1.0                 # w=1 keeps the rasterizer's own
//                                        #   divide a no-op, so attribute
//                                        #   interpolation stays linear in
//                                        #   screen space, exactly the D1/D7
//                                        #   pre-projected behaviour
//
// The fragment stage computes `key + fill` diffuse lighting from the
// interpolated view-space normal so flat vs smooth normals and the
// transparency alpha reach the pixels byte-identically to the pre-D8 path.
//
// The light rig values are injected at build time from `crate::gpu::shading`
// (`shading_wgsl()`), so the WGSL and the Rust constants are one source of
// truth. The placeholders are the only thing substituted; nothing else in this
// file may change.

struct Uniforms {
    // Column-major mat4 (view-projection), packed on the host from
    // `Mat4::to_array16()` as 16 f32; a uniform mat4x4 in WGSL is stored
    // column-by-column, matching that layout.
    mvp: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> u: Uniforms;

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
    // World -> homogeneous clip, then the vertex stage does the perspective
    // divide; w=1 is output so the rasterizer's own divide is a no-op and the
    // pre-D8 screen-space-linear interpolation semantics are preserved exactly.
    let clip = u.mvp * vec4<f32>(in.pos, 1.0);
    // Perspective divide, then REVERSE-Z: negate the NDC z so the near plane
    // (NDC z = -1) maps to depth 1.0 and far (NDC z = +1) to depth 0.0;
    // the reverse-Z convention the depth attachment uses (UI-D11). x/y are
    // untouched, so raster coverage is bit-for-bit the forward-Z position and
    // the CPU/GPU coverage band is unchanged; only the depth ordering flips.
    // wgpu clip volume is z in [0,1] (probe-verified). Reverse-Z: map NDC z
    // so near (nz=-1) -> 1.0 and far (nz=+1) -> 0.0, staying inside [0,1],
    // with GreaterEqual + clear 0.0 (far). Precision sits at the near plane.
    let ndc = clip.xyz / clip.w;
    out.clip = vec4<f32>(ndc.x, ndc.y, 0.5 - 0.5 * ndc.z, 1.0);
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
