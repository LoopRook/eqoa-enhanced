// The client's map (render/src/map.rs): quads in clip space, a texture with
// its mip chain sampled trilinearly, times a colour. Alpha blended.

struct VsIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) circle: vec4<f32>,
};

struct VsOut {
    @builtin(position) clip: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) circle: vec4<f32>,
};

@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

@vertex
fn vs_main(v: VsIn) -> VsOut {
    var o: VsOut;
    o.clip = vec4<f32>(v.pos, 0.0, 1.0);
    o.uv = v.uv;
    o.color = v.color;
    o.circle=vec4<f32>((v.pos-v.circle.xy)*v.circle.zw,select(0.0,1.0,v.circle.z>0.0),0.0);
    return o;
}

@fragment
fn fs_main(i: VsOut) -> @location(0) vec4<f32> {
    let c=textureSample(tex,samp,i.uv)*i.color;
    if (i.circle.z>0.0 && length(i.circle.xy)>1.0) {discard;}
    return c;
}
