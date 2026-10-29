//! Inactive preparation: activate only after compiler handoff; no results claimed.
use super::*;
use dear_imgui_rs::Context;

fn context() -> Context {
    let mut c = Context::create();
    c.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    c.io_mut().set_display_size([256., 192.]);
    c.io_mut().set_delta_time(1. / 60.);
    c.font_atlas().try_claim_legacy_renderer().unwrap().build();
    c
}

#[test]
fn forge_drawlist_handles_reject_ended_child_reopened_window_and_previous_frame() {
    let mut c = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    let line = "saved:AddLine(ImVec2.new(0,0),ImVec2.new(5,5),-1)";
    for (setup, finish) in [
        (
            "ImGui.Begin('a');saved=ImGui.GetWindowDrawList();ImGui.End()",
            "",
        ),
        (
            "ImGui.Begin('a');saved=ImGui.GetWindowDrawList();ImGui.End();ImGui.Begin('a')",
            "ImGui.End()",
        ),
        (
            "ImGui.Begin('a');ImGui.BeginChild('c',60,40);saved=ImGui.GetWindowDrawList();ImGui.EndChild()",
            "ImGui.End()",
        ),
        (
            "ImGui.Begin('a');saved=ImGui.GetWindowDrawList();ImGui.BeginChild('c',60,40)",
            "ImGui.EndChild();ImGui.End()",
        ),
    ] {
        c.frame();
        let result = b.invoke(|| {
            h.execute(setup)?;
            h.execute(line)
        });
        assert!(
            result.unwrap_err().to_string().contains("window"),
            "{setup}"
        );
        // invoke unwinds the unfinished scopes on failure; no fake End in Lua.
        assert!(b.frame.borrow().stack.is_empty(), "{finish}");
        drop(c.render_legacy());
    }
    c.frame();
    b.invoke(|| h.execute("saved=ImGui.GetBackgroundDrawList()"))
        .unwrap();
    drop(c.render_legacy());
    c.frame();
    assert!(
        b.invoke(|| h.execute(line))
            .unwrap_err()
            .to_string()
            .contains("expired")
    );
    drop(c.render_legacy());
    assert!(h.execute(line).is_err());
}

#[test]
fn forge_drawlist_rejects_foreign_context_and_callback_window_handles_before_ffi() {
    let mut first = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    first.frame();
    b.invoke(|| h.execute("ImGui.Begin('a');saved=ImGui.GetWindowDrawList();ImGui.End()"))
        .unwrap();
    drop(first.render_legacy());
    drop(first);
    // Fresh Context can reuse frame counts and even allocator addresses: window
    // invocation identity must still prevent a cached handle becoming valid.
    let mut second = context();
    second.frame();
    assert!(
        b.invoke(|| h.execute(
            "ImGui.Begin('a');saved:AddRectFilled(ImVec2.new(1,1),ImVec2.new(5,5),-1);ImGui.End()"
        ))
        .is_err()
    );
    drop(second.render_legacy());
}

#[test]
fn forge_drawlist_geometry_budget_is_shared_across_invocations_in_one_frame() {
    let mut c = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    c.frame();
    // Each 512-segment outlined primitive reserves 12 index units per point.
    let batch = "local d=ImGui.GetBackgroundDrawList();for i=1,10 do d:AddCircle(ImVec2.new(30,30),20,-1,512,2) end";
    b.invoke(|| h.execute(batch)).unwrap();
    let err = b.invoke(|| h.execute(batch)).unwrap_err();
    assert!(err.to_string().contains("geometry budget"));
    drop(c.render_legacy());
    c.frame();
    b.invoke(|| h.execute(batch)).unwrap();
    drop(c.render_legacy());
}

#[test]
fn forge_drawlist_one_hundred_realistic_auto_balls_fit_but_large_geometry_is_bounded() {
    let mut c = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    c.frame();
    // Unchanged uiforge_example PopulateBalls uses radius10..20 and its
    // settings permit100 balls. Exercise the worst radius of that real range.
    b.invoke(||h.execute("local d=ImGui.GetBackgroundDrawList();for i=1,100 do d:AddCircleFilled(ImVec2.new(80,80),20,-1) end")).unwrap();
    drop(c.render_legacy());
    c.frame();
    let err=b.invoke(||h.execute("local d=ImGui.GetBackgroundDrawList();for i=1,100 do d:AddCircleFilled(ImVec2.new(80,80),4096,-1) end")).unwrap_err();
    assert!(err.to_string().contains("geometry budget"));
    drop(c.render_legacy());
}

#[test]
fn forge_drawlist_argument_bounds_and_signed_packed_colors() {
    let mut c = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    for args in [
        "d:AddCircle(v,20,-1,2)",
        "d:AddCircle(v,20,-1,513)",
        "d:AddNgon(v,20,-1,0)",
        "d:AddNgonFilled(v,-1,-1,4)",
        "d:AddRect(v,v,-1,0,1,1)", // unsupported rectangle flag, before native assertion
        "d:AddRect(v,v,-1,0,0,65)",
        "d:AddRectFilled(v,v,-1,0/0)",
        "d:AddTriangleFilled(v,v,v,4294967296)",
        "d:AddLine(v,v,-2147483649)",
    ] {
        c.frame();
        assert!(
            b.invoke(|| h.execute(&format!(
                "local d=ImGui.GetBackgroundDrawList();local v=ImVec2.new(10,10);{args}"
            )))
            .is_err(),
            "{args}"
        );
        drop(c.render_legacy());
    }
    c.frame();
    b.invoke(||h.execute("local d=ImGui.GetBackgroundDrawList();local v=ImVec2.new(10,10);d:AddRectFilled(v,ImVec2.new(20,20),-16776961);d:AddCircle(v,2,4294967295);d:AddNgonFilled(v,2,-1,3)")).unwrap();
    drop(c.render_legacy());
}

#[test]
fn forge_drawlist_callback_scope_destroyed_userdata_and_context_mismatch() {
    let mut c = context();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    c.frame();
    // Host-owned settings window stays open across callbacks, so ID/depth and
    // BeginCount alone cannot establish Lua callback scope lifetime.
    let label = std::ffi::CString::new("host-owned").unwrap();
    unsafe {
        sys::igBegin(label.as_ptr(), std::ptr::null_mut(), 0);
    }
    let acquired = b.invoke(|| h.execute("saved=ImGui.GetWindowDrawList()"));
    let reused = if acquired.is_ok() {
        b.invoke(|| h.execute("saved:AddCircleFilled(ImVec2.new(10,10),3,-1)"))
    } else {
        Err(mlua::Error::runtime("getter unavailable"))
    };
    unsafe {
        sys::igEnd();
    }
    acquired.unwrap();
    assert!(reused.unwrap_err().to_string().contains("window"));
    b.invoke(|| h.execute("saved=ImGui.GetBackgroundDrawList()"))
        .unwrap();
    let mismatch = b.invoke(|| {
        let owned = unsafe { sys::igGetCurrentContext() };
        unsafe {
            sys::igSetCurrentContext(std::ptr::null_mut());
        }
        let result = h.execute("saved:AddLine(ImVec2.new(0,0),ImVec2.new(5,5),-1)");
        unsafe {
            sys::igSetCurrentContext(owned);
        }
        result
    });
    assert!(mismatch.is_err());
    h.lua
        .globals()
        .get::<AnyUserData>("saved")
        .unwrap()
        .destroy()
        .unwrap();
    assert!(
        b.invoke(|| h.execute("saved:AddLine(ImVec2.new(0,0),ImVec2.new(5,5),-1)"))
            .is_err()
    );
    drop(c.render_legacy());
}

// One common GPU harness for actual primitives, clipping, and failed-owner
// recovery. No mock renderer and no success inferred from draw command counts.
fn gpu_case(
    mut draw: impl FnMut(&Host, &Bridge, &Host, &Bridge),
    mut inspect: impl FnMut(&[u8], &Host),
) {
    use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
    use render::wgpu;
    let instance = wgpu::Instance::default();
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIP forge drawlist GPU: no adapter: {e}");
            return;
        }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut c = Context::create();
    c.set_ini_filename::<std::path::PathBuf>(None).unwrap();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        &mut c,
    )
    .unwrap();
    let h = Host::new().unwrap();
    let b = Bridge::install(&h).unwrap();
    let n = Host::new().unwrap();
    let nb = Bridge::install(&n).unwrap();
    // Warm up native auto-fit and window visibility. Inspect only stable frame 3.
    for frame in 0..3 {
        c.io_mut().set_display_size([256., 192.]);
        c.io_mut().set_delta_time(1. / 60.);
        c.frame();
        draw(&h, &b, &n, &nb);
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("drawlist oracle"),
            size: wgpu::Extent3d {
                width: 256,
                height: 192,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = target.create_view(&Default::default());
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let rendered = c.render(renderer.renderer_consumer().unwrap());
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    depth_slice: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            renderer
                .render(rendered, &mut pass, FramebufferExtent::new(256, 192))
                .unwrap();
        }
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: 256 * 192 * 4,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(1024),
                    rows_per_image: Some(192),
                },
            },
            target.size(),
        );
        queue.submit([encoder.finish()]);
        let (send, recv) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        recv.recv().unwrap().unwrap();
        let bytes = buffer.slice(..).get_mapped_range().unwrap();
        if frame == 2 {
            inspect(&bytes, &h);
        }
    }
}
fn pixel(p: &[u8], x: usize, y: usize) -> [u8; 4] {
    p[(y * 256 + x) * 4..][..4].try_into().unwrap()
}
fn red(p: [u8; 4]) -> bool {
    p[0] > 220 && p[1] < 30 && p[2] < 30
}
fn black(p: [u8; 4]) -> bool {
    p[0] < 20 && p[1] < 20 && p[2] < 20
}

#[test]
fn forge_drawlist_all_shape_families_have_distinct_fill_outline_exterior_gpu() {
    gpu_case(
        |h, b, _, _| {
            b.invoke(|| {
                h.execute(
                    r#"
        local d=ImGui.GetBackgroundDrawList();local v=ImVec2.new;local r=-16776961
        d:AddRect(v(10,10),v(42,42),r,0,0,4)
        d:AddRectFilled(v(10,60),v(42,92),r)
        d:AddCircle(v(80,26),16,r,64,4);d:AddCircleFilled(v(80,76),16,r,64)
        d:AddTriangle(v(110,42),v(126,10),v(142,42),r,4)
        d:AddTriangleFilled(v(110,92),v(126,60),v(142,92),r)
        d:AddNgon(v(180,26),16,r,4,4);d:AddNgonFilled(v(180,76),16,r,4)
    "#,
                )
            })
            .unwrap()
        },
        |p, _| {
            for (x, y) in [(26, 26), (80, 26), (126, 30), (180, 26)] {
                assert!(black(pixel(p, x, y)), "outline interior {x},{y}");
            }
            for (x, y) in [(10, 26), (95, 26), (126, 42), (195, 26)] {
                assert!(
                    red(pixel(p, x, y)),
                    "outline edge {x},{y}: {:?}",
                    pixel(p, x, y)
                );
            }
            for (x, y) in [(26, 76), (80, 76), (126, 80), (180, 76)] {
                assert!(red(pixel(p, x, y)), "filled interior {x},{y}");
            }
            for (x, y) in [(4, 26), (103, 26), (150, 30), (204, 26)] {
                assert!(black(pixel(p, x, y)), "exterior {x},{y}");
            }
        },
    );
}

#[test]
fn forge_drawlist_child_clip_differs_from_background_gpu() {
    gpu_case(
        |h, b, _, _| {
            b.invoke(|| {
                unsafe {
                    sys::igSetNextWindowPos(
                        sys::ImVec2_c { x: 10., y: 10. },
                        sys::ImGuiCond_Always as i32,
                        sys::ImVec2_c { x: 0., y: 0. },
                    );
                    sys::igSetNextWindowSize(
                        sys::ImVec2_c { x: 210., y: 150. },
                        sys::ImGuiCond_Always as i32,
                    );
                }
                h.execute(
                    r#"
            ImGui.Begin('clip',true,ImGuiWindowFlags.NoDecoration)
            ImGui.BeginChild('clipchild',80,60)
            local x,y=ImGui.GetCursorScreenPos();clipx=x;clipy=y
            local v=ImVec2.new
            ImGui.GetWindowDrawList():AddRectFilled(v(x,y),v(x+120,y+15),-16776961)
            ImGui.GetBackgroundDrawList():AddRectFilled(v(230,y),v(245,y+15),-16776961)
            ImGui.EndChild();ImGui.End()
        "#,
                )
            })
            .unwrap()
        },
        |p, h| {
            let x = h.lua.globals().get::<f32>("clipx").unwrap() as usize;
            let y = h.lua.globals().get::<f32>("clipy").unwrap() as usize;
            assert!(red(pixel(p, x + 9, y + 6)), "child interior");
            assert!(
                !red(pixel(p, x + 100, y + 6)),
                "window list must be clipped to child"
            );
            assert!(
                red(pixel(p, 237, y + 6)),
                "background list must remain unclipped"
            );
        },
    );
}

#[test]
fn forge_drawlist_failed_owner_unwinds_and_healthy_neighbor_gpu_survives() {
    gpu_case(
        |h, b, n, nb| {
            assert!(b.invoke(||h.execute("ImGui.Begin('bad');ImGui.BeginChild('c',60,30);local d=ImGui.GetWindowDrawList();ImGui.EndChild();d:AddCircleFilled(ImVec2.new(10,10),5,-1);ImGui.End()")).is_err());
            assert!(b.frame.borrow().stack.is_empty());
            nb.invoke(||n.execute("ImGui.GetBackgroundDrawList():AddRectFilled(ImVec2.new(230,160),ImVec2.new(250,180),-16711936)")).unwrap();
        },
        |p, _| {
            let v = pixel(p, 240, 170);
            assert!(v[0] < 30 && v[1] > 220 && v[2] < 30, "{v:?}");
        },
    );
}
