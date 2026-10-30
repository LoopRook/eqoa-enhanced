use super::*;
use dear_imgui_rs::Context;

fn context() -> Context {
    let mut context = Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    context.io_mut().set_display_size([640.0, 480.0]);
    context.io_mut().set_delta_time(1.0 / 60.0);
    context
        .font_atlas()
        .try_claim_legacy_renderer()
        .unwrap()
        .build();
    context
}

#[test]
fn forge_bridge_next_begin_overload_open_precedes_draw_and_always_requires_end() {
    let mut context = context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    // New windows auto-fit for their initial frames even when collapsed.
    // Establish the actual native window before asserting collapsed visibility.
    for _ in 0..2 {
        context.frame();
        bridge
            .invoke(|| host.execute("ImGui.Begin('collapsed',true);ImGui.End()"))
            .unwrap();
        drop(context.render_legacy());
    }
    context.frame();
    unsafe {
        sys::igSetNextWindowCollapsed(true, sys::ImGuiCond_Always as i32);
    }
    let script=host.compile("local open,draw=ImGui.Begin('collapsed',true);assert(open==true and draw==false,'Begin must return open,shouldDraw: '..tostring(open)..'/'..tostring(draw));ImGui.End()").unwrap();
    let result = bridge.invoke(|| host.run(&script));
    // Render after the guarded invocation even on RED failure: all scopes must
    // already have been unwound before the next native frame.
    drop(context.render_legacy());
    result.unwrap();
    context.frame();
    unsafe {
        sys::igSetNextWindowCollapsed(false, sys::ImGuiCond_Always as i32);
    }
    let script=host.compile("local open,draw=ImGui.Begin('closed-input',false);assert(open==false and type(draw)=='boolean');ImGui.End();assert(select('#',ImGui.Begin('one-return'))==1);ImGui.End()").unwrap();
    bridge.invoke(|| host.run(&script)).unwrap();
    drop(context.render_legacy());
    context.frame();
    let script = host.compile("ImGui.Begin('missing-End',false)").unwrap();
    assert!(
        bridge
            .invoke(|| host.run(&script))
            .unwrap_err()
            .to_string()
            .contains("unbalanced")
    );
    drop(context.render_legacy());
}

#[test]
fn forge_bridge_next_common_widget_defaults_returns_and_native_scopes() {
    let mut context = context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let script=host.compile(r#"
        ImGui.Begin('common widgets')
        assert(type(ImGui.Button('sized',80,20))=='boolean')
        assert(type(ImGui.Button('default'))=='boolean')
        assert(type(ImGui.RadioButton('bool',false))=='boolean')
        local n,c=ImGui.RadioButton('int',2,3);assert(n==2 and c==false)
        n,c=ImGui.Combo('combo',0,{'One','Two'},2);assert(n==0 and c==false)
        n,c=ImGui.Combo('missing item',1,{'One'},2,4);assert(n==1 and c==false)
        n,c=ImGui.InputInt('input',7);assert(n==7 and c==false)
        ImGui.BulletText('literal %n %s');ImGui.TextColored(1,0,0,1,'red %n');ImGui.TextDisabled('disabled %f')
        ImGui.TextWrapped('wrap %d');ImGui.TextUnformatted('raw %s')
        ImGui.PushTextWrapPos();ImGui.TextWrapped('bounded');ImGui.PopTextWrapPos()
        ImGui.Separator();ImGui.Dummy(4,4)
        ImGui.PushStyleColor(ImGuiCol.PlotHistogram,0,1,0,1)
        ImGui.ProgressBar(0.5);ImGui.ProgressBar(-0.4*ImGui.GetTime(),80,12,'50% %n');ImGui.PopStyleColor()
        assert(type(ImGui.IsItemHovered())=='boolean' and type(ImGui.IsItemActive())=='boolean')
        ImGui.SetTooltip('literal %n');ImGui.SetKeyboardFocusHere(0)
        local open,draw=ImGui.CollapsingHeader('header',true,ImGuiTreeNodeFlags.DefaultOpen)
        assert(open==true and type(draw)=='boolean')
        assert(type(ImGui.CollapsingHeader('header flags',0))=='boolean')
        assert(type(ImGui.BeginChild('child',100,50,ImGuiChildFlags.Border))=='boolean')
        ImGui.Text('child body');ImGui.EndChild()
        if ImGui.BeginTabBar('tabs') then
            local open,draw=ImGui.BeginTabItem('tab',true);assert(open==true and type(draw)=='boolean')
            if draw then ImGui.Text('tab body');ImGui.EndTabItem() end
            ImGui.EndTabBar()
        end
        ImGui.End()
        assert(ImGuiWindowFlags.NoNav==ImGuiWindowFlags.NoNavInputs+ImGuiWindowFlags.NoNavFocus)
        assert(ImGuiWindowFlags.NoDecoration==ImGuiWindowFlags.NoTitleBar+ImGuiWindowFlags.NoResize+ImGuiWindowFlags.NoScrollbar+ImGuiWindowFlags.NoCollapse)
        assert(ImGuiWindowFlags.NoInputs==ImGuiWindowFlags.NoMouseInputs+ImGuiWindowFlags.NoNav)
        ImGui.Begin('menu widgets',true,ImGuiWindowFlags.MenuBar)
        if ImGui.BeginMenuBar() then
            assert(ImGui.BeginMenu('disabled',false)==false)
            local selected,activated=ImGui.MenuItem('item','Ctrl+X',true,false)
            assert(selected==true and activated==false)
            assert(type(ImGui.MenuItem('one return'))=='boolean')
            ImGui.EndMenuBar()
        end
        ImGui.End()
        if ImGui.BeginMainMenuBar() then ImGui.MenuItem('main');ImGui.EndMainMenuBar() end
    "#).unwrap();
    context.frame();
    let result = bridge.invoke(|| host.run(&script));
    drop(context.render_legacy());
    result.unwrap();
    context.frame();
    bridge.invoke(|| {
        host.execute("ImGui.Begin('tree')")?;
        unsafe {sys::igSetNextItemOpen(true,sys::ImGuiCond_Always as i32);}
        host.execute("assert(ImGui.TreeNode('id','literal %n'));ImGui.Text('node');ImGui.TreePop();ImGui.End()")
    }).unwrap();
    drop(context.render_legacy());
}

#[test]
fn forge_bridge_next_mismatched_closed_unclosed_scopes_and_quotas_recover() {
    let mut context = context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    let second = Host::new().unwrap();
    let healthy = Bridge::install(&second).unwrap();
    for source in [
        "ImGui.End()",
        "ImGui.Begin('parent');ImGui.BeginChild('child',80,30);ImGui.End()",
        "ImGui.Begin('parent');ImGui.BeginChild('child',80,30)",
        "ImGui.Begin('parent');ImGui.BeginTabItem('orphan')",
        "ImGui.Begin('parent');if ImGui.BeginTabBar('bar') then ImGui.End() end",
        "ImGui.Begin('parent');if ImGui.BeginTabBar('bar') then local o,d=ImGui.BeginTabItem('item',true);if d then ImGui.EndTabBar() end end",
        "ImGui.Begin('parent');ImGui.PushTextWrapPos();ImGui.End()",
        "ImGui.Begin('parent');assert(not ImGui.BeginMenu('disabled',false));ImGui.EndMenu()",
        "ImGui.Begin('parent');assert(not ImGui.TreeNode('closed node'));ImGui.TreePop()",
        "ImGui.Begin('parent');ImGui.PushStyleColor(ImGuiCol.Text,1,0,0,1);ImGui.BeginChild('child',80,30);ImGui.PopStyleColor()",
        "ImGui.Begin('parent');ImGui.PushTextWrapPos();ImGui.BeginChild('child',80,30);ImGui.PopTextWrapPos()",
        "ImGui.Begin('parent');ImGui.InputInt('bad',0,1,100,4194304)", // callback flag with no callback must never reach native
        "ImGui.Begin('parent');ImGui.Combo('huge',0,{},257)",
        "ImGui.Begin('parent');ImGui.Button('nan',0/0,1)",
        "ImGui.Begin('parent');ImGui.TextUnformatted('text','unrelated-end')",
        "ImGui.Begin('parent');for i=1,40 do ImGui.PushTextWrapPos() end",
        "ImGui.Begin('parent');for i=1,5000 do ImGui.Separator() end",
    ] {
        context.frame();
        let script = host.compile(source).unwrap();
        let error = bridge.invoke(|| host.run(&script)).expect_err(source);
        assert!(!error.to_string().is_empty());
        healthy
            .invoke(|| {
                second.execute("ImGui.Begin('healthy neighbor');ImGui.Text('survived');ImGui.End()")
            })
            .unwrap();
        drop(context.render_legacy());
        assert!(bridge.frame.borrow().stack.is_empty());
        assert!(bridge.frame.borrow().context.is_null());
    }
    // A collapsed child still owns a mandatory EndChild, unlike a closed tree,
    // menu or tab whose false Begin creates no matching End obligation.
    for _ in 0..2 {
        context.frame();
        bridge
            .invoke(|| host.execute("ImGui.Begin('collapse parent',true);ImGui.End()"))
            .unwrap();
        drop(context.render_legacy());
    }
    context.frame();
    unsafe {
        sys::igSetNextWindowCollapsed(true, sys::ImGuiCond_Always as i32);
    }
    bridge.invoke(||host.execute("local o,d=ImGui.Begin('collapse parent',true);assert(o and not d);assert(not ImGui.BeginChild('hidden child',80,30));ImGui.EndChild();ImGui.End()")).unwrap();
    drop(context.render_legacy());
    assert!(
        bridge
            .invoke(|| host.execute("ImGui.Button('outside native frame')"))
            .is_err()
    );
    assert!(bridge.frame.borrow().context.is_null());
    context.frame();
    let duplicate=bridge.invoke(||host.execute("ImGui.Begin('duplicate menu',true,ImGuiWindowFlags.MenuBar);assert(ImGui.BeginMenuBar());ImGui.BeginMenuBar()"));
    assert!(
        duplicate
            .unwrap_err()
            .to_string()
            .contains("already active")
    );
    drop(context.render_legacy());
}

#[test]
fn forge_bridge_next_real_mouse_and_keyboard_change_widget_returns() {
    let mut context = context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();
    host.lua
        .globals()
        .set(
            "input_flags",
            sys::ImGuiInputTextFlags_AutoSelectAll | sys::ImGuiInputTextFlags_EnterReturnsTrue,
        )
        .unwrap();
    let script=host.compile(r#"
        ImGui.Begin('interaction',true,ImGuiWindowFlags.NoTitleBar)
        bx,by=ImGui.GetCursorScreenPos();clicked=ImGui.Button('click',100,24)
        hovered=ImGui.IsItemHovered();active=ImGui.IsItemActive()
        rx,ry=ImGui.GetCursorScreenPos();radio,radio_changed=ImGui.RadioButton('choice',radio or 0,1)
        if focus then ImGui.SetKeyboardFocusHere();focus=false end
        value,input_changed=ImGui.InputInt('value',value or 7,0,0,input_flags)
        ImGui.End()
    "#).unwrap();
    let run = |context: &mut Context| {
        context.frame();
        unsafe {
            sys::igSetNextWindowPos(
                Vec2(20., 20.).raw(),
                sys::ImGuiCond_Always as i32,
                Vec2(0., 0.).raw(),
            );
            sys::igSetNextWindowSize(Vec2(320., 240.).raw(), sys::ImGuiCond_Always as i32);
        }
        bridge.invoke(|| host.run(&script)).unwrap();
        drop(context.render_legacy());
    };
    run(&mut context);
    run(&mut context);
    let xy = |x: &str, y: &str| {
        [
            host.lua.globals().get::<f32>(x).unwrap() + 5.,
            host.lua.globals().get::<f32>(y).unwrap() + 5.,
        ]
    };
    context.io_mut().add_mouse_pos_event(xy("bx", "by"));
    run(&mut context);
    assert!(host.lua.globals().get::<bool>("hovered").unwrap());
    context
        .io_mut()
        .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    run(&mut context);
    assert!(host.lua.globals().get::<bool>("active").unwrap());
    context
        .io_mut()
        .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    run(&mut context);
    assert!(
        host.lua.globals().get::<bool>("clicked").unwrap(),
        "real release must activate Button"
    );
    run(&mut context);
    assert!(!host.lua.globals().get::<bool>("clicked").unwrap());
    context.io_mut().add_mouse_pos_event(xy("rx", "ry"));
    run(&mut context);
    context
        .io_mut()
        .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, true);
    run(&mut context);
    context
        .io_mut()
        .add_mouse_button_event(dear_imgui_rs::MouseButton::Left, false);
    run(&mut context);
    assert_eq!(host.lua.globals().get::<i32>("radio").unwrap(), 1);
    assert!(host.lua.globals().get::<bool>("radio_changed").unwrap());
    host.lua.globals().set("focus", true).unwrap();
    run(&mut context);
    run(&mut context);
    context.io_mut().add_input_characters_utf8("42");
    run(&mut context);
    context
        .io_mut()
        .add_key_event(dear_imgui_rs::Key::Enter, true);
    run(&mut context);
    assert_eq!(
        host.lua.globals().get::<i32>("value").unwrap(),
        42,
        "actual text+Enter must update InputInt"
    );
    assert!(host.lua.globals().get::<bool>("input_changed").unwrap());
    context
        .io_mut()
        .add_key_event(dear_imgui_rs::Key::Enter, false);
    run(&mut context);
}

#[test]
fn forge_bridge_next_scope_failure_unwinds_before_healthy_neighbor_gpu_readback() {
    use dear_imgui_wgpu::{FramebufferExtent, WgpuInitInfo, WgpuRenderer};
    use render::wgpu;
    let instance = wgpu::Instance::default();
    let adapter = match pollster::block_on(instance.request_adapter(&Default::default())) {
        Ok(a) => a,
        Err(e) => {
            eprintln!("SKIP bridge scope recovery GPU: {e}");
            return;
        }
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let mut context = Context::create();
    context
        .set_ini_filename::<std::path::PathBuf>(None)
        .unwrap();
    let mut renderer = WgpuRenderer::new(
        WgpuInitInfo::new(
            device.clone(),
            queue.clone(),
            wgpu::TextureFormat::Rgba8Unorm,
        ),
        &mut context,
    )
    .unwrap();
    let victim = Host::new().unwrap();
    let victim_bridge = Bridge::install(&victim).unwrap();
    let neighbor = Host::new().unwrap();
    let neighbor_bridge = Bridge::install(&neighbor).unwrap();
    let good=neighbor.compile("ImGui.GetBackgroundDrawList():AddLine(ImVec2.new(0,40),ImVec2.new(256,40),ImGui.GetColorU32(0,1,0,1),6);ImGui.Begin('neighbor');ImGui.Text('healthy');ImGui.End()").unwrap();
    for source in [
        "ImGui.Begin('victim');ImGui.BeginChild('child',80,60);ImGui.PushTextWrapPos();error('controlled child')",
        "ImGui.Begin('victim');if ImGui.BeginTabBar('bar') then local o,d=ImGui.BeginTabItem('item',true);if d then ImGui.PushStyleColor(ImGuiCol.Text,1,0,0,1);error('controlled tab') end end",
        "ImGui.Begin('victim');ImGui.PushStyleColor(ImGuiCol.Text,1,0,0,1);ImGui.BeginChild('child',80,60);ImGui.PopStyleColor()",
        "ImGui.Begin('victim');assert(not ImGui.BeginMenu('disabled',false));ImGui.EndMenu()",
    ] {
        context.io_mut().set_display_size([256., 192.]);
        context.io_mut().set_delta_time(1. / 60.);
        context.frame();
        let bad = victim.compile(source).unwrap();
        assert!(victim_bridge.invoke(|| victim.run(&bad)).is_err());
        assert!(victim_bridge.frame.borrow().stack.is_empty());
        neighbor_bridge.invoke(|| neighbor.run(&good)).unwrap();
        let target = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scope recovery"),
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
            let frame = context.render(renderer.renderer_consumer().unwrap());
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
                .render(frame, &mut pass, FramebufferExtent::new(256, 192))
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
        let pixels = buffer.slice(..).get_mapped_range().unwrap();
        let pixel = &pixels[(40 * 256 + 128) * 4..][..4];
        assert!(
            pixel[0] < 30 && pixel[1] > 220 && pixel[2] < 30,
            "healthy neighbor must render after {source}: {pixel:?}"
        );
    }
}

#[test]
fn forge_bridge_wereoxxs_widgets_and_stack_unwinding() {
    let mut context = context();
    let host = Host::new().unwrap();
    let bridge = Bridge::install(&host).unwrap();

    // Frame 1: Exercise new widgets inside window
    context.frame();
    let script = host.compile(r#"
        assert(ImGuiCond.FirstUseEver == 4, 'ImGuiCond.FirstUseEver missing')
        assert(ImGuiCol.FrameBg ~= nil, 'ImGuiCol.FrameBg missing')
        assert(ImGuiCol.FrameBgHovered ~= nil, 'ImGuiCol.FrameBgHovered missing')
        assert(ImGuiCol.Tab ~= nil, 'ImGuiCol.Tab missing')

        ImGui.SetNextWindowPos(10, 20, ImGuiCond.Always)
        ImGui.SetNextWindowSize(300, 200, ImGuiCond.Always)
        local visible = ImGui.Begin('TestWereoxxsWidgets')
        assert(visible, 'window should be visible')

        local wx, wy = ImGui.GetWindowPos()
        assert(wx >= 0 and wy >= 0, 'GetWindowPos returned invalid')

        local aw, ah = ImGui.GetContentRegionAvail()
        assert(aw > 0 and ah > 0, 'GetContentRegionAvail positive')

        local fh = ImGui.GetFrameHeight()
        assert(fh > 0, 'GetFrameHeight positive')

        local lh = ImGui.GetTextLineHeight()
        assert(lh > 0, 'GetTextLineHeight positive')

        local sy = ImGui.GetScrollY()
        local max_sy = ImGui.GetScrollMaxY()
        assert(sy >= 0 and max_sy >= 0, 'Scroll values non-negative')
        ImGui.SetScrollHereY(1.0)

        ImGui.SetCursorScreenPos(15, 25)
        ImGui.SetNextItemWidth(100)

        local btn = ImGui.InvisibleButton('inv_btn', 50, 20)
        assert(type(btn) == 'boolean')

        local txt, entered = ImGui.InputTextWithHint('##hint_test', 'type here', 'hello', ImGuiInputTextFlags.EnterReturnsTrue)
        assert(txt == 'hello', 'InputTextWithHint returns current text')
        assert(type(entered) == 'boolean')

        -- ID stack push/pop string & int
        ImGui.PushID('string_id')
        ImGui.PushID(42)
        ImGui.PopID()
        ImGui.PopID()

        -- PushStyleColor and multi-pop
        ImGui.PushStyleColor(ImGuiCol.Text, 1.0, 0.0, 0.0, 1.0)
        ImGui.PushStyleColor(ImGuiCol.WindowBg, 0.0, 1.0, 0.0, 1.0)
        ImGui.PopStyleColor(2)

        -- Selectable
        local sel = ImGui.Selectable('Option 1', true)
        assert(type(sel) == 'boolean')

        -- Combo (returns false when closed)
        local combo_open = ImGui.BeginCombo('test_combo', 'preview')
        assert(type(combo_open) == 'boolean')
        if combo_open then
            ImGui.Selectable('item', false)
            ImGui.EndCombo()
        end

        -- Popup
        ImGui.OpenPopup('test_popup')
        if ImGui.BeginPopup('test_popup') then
            ImGui.Text('popup text')
            ImGui.EndPopup()
        end

        -- Mouse queries
        local dragging = ImGui.IsMouseDragging(0)
        assert(type(dragging) == 'boolean')
        local hovering = ImGui.IsMouseHoveringRect(0, 0, 100, 100)
        assert(type(hovering) == 'boolean')
        local focused = ImGui.IsWindowFocused()
        assert(type(focused) == 'boolean')
        local hovered = ImGui.IsWindowHovered()
        assert(type(hovered) == 'boolean')

        ImGui.End()
    "#).unwrap();
    bridge.invoke(|| host.run(&script)).unwrap();
    drop(context.render_legacy());

    // Frame 2: Unbalanced EndCombo when not opened
    context.frame();
    let broken_combo = host
        .compile(
            r#"
        ImGui.Begin('broken_combo')
        ImGui.EndCombo()
    "#,
        )
        .unwrap();
    let err = bridge.invoke(|| host.run(&broken_combo)).unwrap_err();
    assert!(err.to_string().contains("unbalanced"));
    drop(context.render_legacy());

    // Frame 3: Unbalanced EndPopup when not opened
    context.frame();
    let broken_popup = host
        .compile(
            r#"
        ImGui.Begin('broken_popup')
        ImGui.EndPopup()
    "#,
        )
        .unwrap();
    let err = bridge.invoke(|| host.run(&broken_popup)).unwrap_err();
    assert!(err.to_string().contains("unbalanced"));
    drop(context.render_legacy());

    // Frame 4: Unbalanced PopID when nothing pushed
    context.frame();
    let broken_id = host
        .compile(
            r#"
        ImGui.Begin('broken_id')
        ImGui.PopID()
    "#,
        )
        .unwrap();
    let err = bridge.invoke(|| host.run(&broken_id)).unwrap_err();
    assert!(err.to_string().contains("unbalanced"));
    drop(context.render_legacy());

    // Frame 5: Stack unwinding on PushID leak
    context.frame();
    let broken_unwind = host
        .compile(
            r#"
        ImGui.Begin('broken_unwind')
        ImGui.PushID('leak')
        error('simulated unwind error')
    "#,
        )
        .unwrap();
    let err = bridge.invoke(|| host.run(&broken_unwind)).unwrap_err();
    assert!(err.to_string().contains("simulated unwind error"));
    drop(context.render_legacy());

    // Frame 6: Ensure healthy next frame after unwinding all errors
    context.frame();
    let clean = host
        .compile(
            r#"
        ImGui.Begin('clean')
        ImGui.Text('clean frame after unwinding')
        ImGui.End()
    "#,
        )
        .unwrap();
    bridge.invoke(|| host.run(&clean)).unwrap();
    drop(context.render_legacy());
}
