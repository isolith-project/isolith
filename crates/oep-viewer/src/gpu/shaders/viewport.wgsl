// WGSL passthrough shader for the GPU viewport backend (wave D, card UI-D1).
//
// The CPU projects every vertex with the exact view-projection the CPU
// backend uses (including its clip-space convention, so "same scene, same
// view" is numerically identical between backends); this shader only owns the
// rasterization, exactly like an immediate-mode UI (egui itself passes
// screen-space quads to a passthrough vertex stage). No camera uniform, no
// lighting in the fragment stage: the flat face shade factor is computed on
// the host with the same Newell-normal / Lambert math as the CPU rasterizer
// and folded into the vertex colour, so the GPU face colours are byte-faithful
// to the CPU path (the pixels that differ are only raster edges and line
// placement, never the material constants).

struct VsIn {
    @location(0) pos: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    var out: VsOut;
    // `pos` carries already-divided NDC (x/y in [-1,1], z unused); w=1 so the
    // rasterizer's own divide is a no-op.
    out.clip = vec4<f32>(in.pos.x, in.pos.y, 0.0, 1.0);
    out.color = in.color;
    return out;
}

@fragment
fn fs_color(in: VsOut) -> @location(0) vec4<f32> {
    return in.color;
}
